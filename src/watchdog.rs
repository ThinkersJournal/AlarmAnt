//! Observer A of the watchdog (docs/SYNAPSE-ADAPTER.md section 5): the server-side dead-man. It reads
//! the bridge's heartbeats and says when a human should be paged. Pure logic on a clock the caller
//! passes in; no I/O, no learning, no score: every number is a fixed deadline (PM ruling, board 490 C;
//! PATENT-SYNAPSE-READING.md; PATENT-SWEEP constraints S4, S6, S7).
//!
//! What it never does: feed "device offline" into escalation, probe the phone, negotiate a cycle, or
//! treat `alarmant-pc`'s polls as a signal. Its only output is a fixed-text page for the normal chain.

use serde_json::Value;

/// Seconds between the bridge's heartbeats (fixed, not negotiated).
pub const BEAT_SECS: u64 = 60;
/// Consecutive misses (or bad beats) that page, and consecutive good beats that clear.
pub const THRESHOLD: u32 = 3;
/// No new page for a component this long after a human acknowledges it.
pub const QUIET_AFTER_ACK: u64 = 30 * 60;
/// Overall cap on watchdog pages in any rolling hour.
pub const PAGES_PER_HOUR: usize = 2;
/// The longest a maintenance pause may be, in minutes.
pub const MAX_PAUSE_MINUTES: u64 = 60;
/// The page title, always.
pub const TITLE: &str = "AlarmAnt watchdog";

const HOUR: u64 = 3600;

/// What the watchdog watches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Component {
    /// The Synapse machine: its bridge stopped reporting.
    Pc,
    /// Synapse itself: the daemon is down or the canary did not come back.
    Synapse,
}

/// The fixed-shape heartbeat body `{"synapse":"up|down","canary":"ok|late","bridge":"up"}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Heartbeat {
    pub synapse_up: bool,
    pub canary_ok: bool,
}

/// Parses the heartbeat body strictly: exactly the three fields, only the allowed values.
pub fn parse_heartbeat(body: &str) -> Option<Heartbeat> {
    let v: Value = serde_json::from_str(body).ok()?;
    let o = v.as_object()?;
    if o.len() != 3 {
        return None;
    }
    let synapse_up = match o.get("synapse")?.as_str()? {
        "up" => true,
        "down" => false,
        _ => return None,
    };
    let canary_ok = match o.get("canary")?.as_str()? {
        "ok" => true,
        "late" => false,
        _ => return None,
    };
    if o.get("bridge")?.as_str()? != "up" {
        return None;
    }
    Some(Heartbeat {
        synapse_up,
        canary_ok,
    })
}

/// Why a due page was held back (audited once per hold, never sent).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Why {
    /// Two watchdog pages already went out in the last hour.
    HourlyCap,
    /// A maintenance pause is running.
    Paused,
}

/// What the watchdog asks its caller to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Raise this page through the normal alert path.
    Page {
        component: Component,
        title: &'static str,
        body: String,
    },
    /// A page was due and was held back; write it to the audit log.
    Suppressed { component: Component, why: Why },
}

/// Why a pause was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseError {
    /// Zero minutes, or more than [`MAX_PAUSE_MINUTES`].
    OutOfRange,
}

#[derive(Debug, Default, Clone)]
struct Track {
    open: bool,
    /// When the fault began (shown in the page).
    since: u64,
    bad: u32,
    good: u32,
    /// The last bad beat said the daemon is down (otherwise the canary was late).
    down: bool,
    /// A page is owed, not before this time.
    page_at: Option<u64>,
    /// The hold-back for the current owed page was already reported.
    noted: bool,
}

/// The state machine. Hold it behind a lock; every method takes the caller's clock.
#[derive(Debug, Clone)]
pub struct Watchdog {
    /// Silence is measured from here when no later beat exists: the start (the restart grace).
    baseline: u64,
    last_beat: Option<u64>,
    pc: Track,
    syn: Track,
    pages: Vec<u64>,
    paused_until: Option<u64>,
}

impl Watchdog {
    /// A watchdog started at `now`: the missed-beat threshold is also the grace after a restart.
    pub fn new(now: u64) -> Self {
        Watchdog {
            baseline: now,
            last_beat: None,
            pc: Track::default(),
            syn: Track::default(),
            pages: Vec::new(),
            paused_until: None,
        }
    }

    /// A heartbeat arrived at `now`.
    pub fn beat(&mut self, now: u64, hb: Heartbeat) -> Vec<Action> {
        let gap_broke = self
            .last_beat
            .is_some_and(|p| now.saturating_sub(p) >= 2 * BEAT_SECS);
        self.last_beat = Some(now);
        self.expire_pause(now);

        // The PC: counts consecutive beats while open.
        if self.pc.open {
            self.pc.good = if gap_broke { 1 } else { self.pc.good + 1 };
            if self.pc.good >= THRESHOLD {
                self.pc = Track::default();
            }
        }

        // Synapse: the beat's own content.
        if hb.synapse_up && hb.canary_ok {
            self.syn.bad = 0;
            if self.syn.open {
                self.syn.good += 1;
                if self.syn.good >= THRESHOLD {
                    self.syn = Track::default();
                }
            }
        } else {
            self.syn.good = 0;
            if self.syn.bad == 0 && !self.syn.open {
                self.syn.since = now;
            }
            self.syn.bad += 1;
            self.syn.down = !hb.synapse_up;
            if self.syn.bad >= THRESHOLD && !self.syn.open {
                self.syn.open = true;
                self.syn.page_at = Some(now);
            }
        }
        self.run_pages(now)
    }

    /// The timer: call at least once a [`BEAT_SECS`].
    pub fn tick(&mut self, now: u64) -> Vec<Action> {
        self.expire_pause(now);
        let since = self.last_beat.unwrap_or(self.baseline).max(self.baseline);
        if !self.pc.open
            && self.paused_until.is_none()
            && now.saturating_sub(since) >= u64::from(THRESHOLD) * BEAT_SECS
        {
            self.pc = Track {
                open: true,
                since,
                page_at: Some(now),
                ..Track::default()
            };
        }
        self.run_pages(now)
    }

    /// A human acknowledged the watchdog's page for `component`: quiet for 30 minutes, then a
    /// fault that persists pages again.
    pub fn acknowledged(&mut self, component: Component, now: u64) {
        let t = self.track(component);
        if t.open {
            t.page_at = Some(now + QUIET_AFTER_ACK);
            t.noted = false;
        }
    }

    /// Starts a maintenance pause of `minutes`; it expires by itself, and the silence clock restarts
    /// when it ends. Returns the time it ends.
    pub fn pause(&mut self, minutes: u64, now: u64) -> Result<u64, PauseError> {
        if minutes == 0 || minutes > MAX_PAUSE_MINUTES {
            return Err(PauseError::OutOfRange);
        }
        let end = now + minutes * 60;
        self.paused_until = Some(end);
        Ok(end)
    }

    /// When the running maintenance pause ends, if one is running.
    pub fn paused_until(&self) -> Option<u64> {
        self.paused_until
    }

    /// Whether a component has an open watchdog alert.
    pub fn is_open(&self, component: Component) -> bool {
        match component {
            Component::Pc => self.pc.open,
            Component::Synapse => self.syn.open,
        }
    }

    fn track(&mut self, c: Component) -> &mut Track {
        match c {
            Component::Pc => &mut self.pc,
            Component::Synapse => &mut self.syn,
        }
    }

    fn expire_pause(&mut self, now: u64) {
        if let Some(end) = self.paused_until
            && now >= end
        {
            self.paused_until = None;
            if !self.pc.open {
                self.baseline = self.baseline.max(end);
            }
        }
    }

    fn run_pages(&mut self, now: u64) -> Vec<Action> {
        let mut out = Vec::new();
        for c in [Component::Pc, Component::Synapse] {
            let due = {
                let t = self.track(c);
                t.open && t.page_at.is_some_and(|at| now >= at)
            };
            if !due {
                continue;
            }
            let hold = if self.paused_until.is_some() {
                Some(Why::Paused)
            } else {
                self.pages.retain(|&p| now < p + HOUR);
                (self.pages.len() >= PAGES_PER_HOUR).then_some(Why::HourlyCap)
            };
            if let Some(why) = hold {
                let t = self.track(c);
                if !t.noted {
                    t.noted = true;
                    out.push(Action::Suppressed { component: c, why });
                }
                continue;
            }
            self.pages.push(now);
            let body = self.text(c);
            let t = self.track(c);
            t.page_at = None;
            t.noted = false;
            out.push(Action::Page {
                component: c,
                title: TITLE,
                body,
            });
        }
        out
    }

    fn text(&self, c: Component) -> String {
        use crate::ratelimit::utc_hhmm;
        match c {
            Component::Pc => format!("The PC stopped reporting at {}", utc_hhmm(self.pc.since)),
            Component::Synapse if self.syn.down => {
                format!("Synapse is down since {}", utc_hhmm(self.syn.since))
            }
            Component::Synapse => format!(
                "Synapse is not delivering mail since {}",
                utc_hhmm(self.syn.since)
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: u64 = 86_400 * 100 + 12 * 3600; // 12:00 UTC
    const GOOD: Heartbeat = Heartbeat {
        synapse_up: true,
        canary_ok: true,
    };
    const DOWN: Heartbeat = Heartbeat {
        synapse_up: false,
        canary_ok: true,
    };
    const LATE: Heartbeat = Heartbeat {
        synapse_up: true,
        canary_ok: false,
    };

    fn pages(a: &[Action]) -> Vec<(Component, String)> {
        a.iter()
            .filter_map(|x| match x {
                Action::Page {
                    component, body, ..
                } => Some((*component, body.clone())),
                _ => None,
            })
            .collect()
    }

    /// Sends `n` heartbeats a minute apart from `start`, returns every action.
    fn beats(w: &mut Watchdog, start: u64, n: u64, hb: Heartbeat) -> Vec<Action> {
        (0..n)
            .flat_map(|i| w.beat(start + i * BEAT_SECS, hb))
            .collect()
    }

    #[test]
    fn heartbeat_parsing_is_strict() {
        let ok = r#"{"synapse":"up","canary":"ok","bridge":"up"}"#;
        assert_eq!(parse_heartbeat(ok), Some(GOOD));
        assert_eq!(
            parse_heartbeat(r#"{"bridge":"up","canary":"late","synapse":"down"}"#),
            Some(Heartbeat {
                synapse_up: false,
                canary_ok: false
            })
        );
        for bad in [
            "",
            "[]",
            "null",
            r#"{"synapse":"up","canary":"ok"}"#,
            r#"{"synapse":"up","canary":"ok","bridge":"up","text":"hi"}"#,
            r#"{"synapse":"maybe","canary":"ok","bridge":"up"}"#,
            r#"{"synapse":"up","canary":"slow","bridge":"up"}"#,
            r#"{"synapse":"up","canary":"ok","bridge":"down"}"#,
            r#"{"synapse":true,"canary":"ok","bridge":"up"}"#,
        ] {
            assert_eq!(parse_heartbeat(bad), None, "{bad}");
        }
    }

    #[test]
    fn two_missed_beats_page_nothing_and_the_third_pages_once() {
        let mut w = Watchdog::new(T0);
        assert!(w.beat(T0 + 10, GOOD).is_empty());
        assert!(w.tick(T0 + 10 + 2 * BEAT_SECS).is_empty(), "two misses");
        assert!(!w.is_open(Component::Pc));
        let a = w.tick(T0 + 10 + 3 * BEAT_SECS);
        assert_eq!(
            pages(&a),
            [(
                Component::Pc,
                "The PC stopped reporting at 12:00 UTC".into()
            )]
        );
        assert!(
            matches!(&a[0], Action::Page { title, .. } if *title == "AlarmAnt watchdog"),
            "fixed title"
        );
        assert!(
            w.tick(T0 + 10 + 4 * BEAT_SECS).is_empty(),
            "one open page per component"
        );
    }

    #[test]
    fn a_restart_has_a_grace_equal_to_the_threshold() {
        let mut w = Watchdog::new(T0);
        assert!(w.tick(T0).is_empty(), "no page at once on a fresh start");
        assert!(w.tick(T0 + 2 * BEAT_SECS + 59).is_empty());
        assert_eq!(pages(&w.tick(T0 + 3 * BEAT_SECS)).len(), 1);
    }

    #[test]
    fn three_bad_beats_in_a_row_page_with_the_right_words() {
        let mut w = Watchdog::new(T0);
        assert!(beats(&mut w, T0, 2, DOWN).is_empty());
        let a = w.beat(T0 + 2 * BEAT_SECS, DOWN);
        assert_eq!(
            pages(&a),
            [(Component::Synapse, "Synapse is down since 12:00 UTC".into())]
        );

        let mut w = Watchdog::new(T0);
        let a = beats(&mut w, T0, 3, LATE);
        assert_eq!(
            pages(&a),
            [(
                Component::Synapse,
                "Synapse is not delivering mail since 12:00 UTC".into()
            )]
        );
    }

    #[test]
    fn a_good_beat_between_bad_ones_resets_the_count() {
        let mut w = Watchdog::new(T0);
        beats(&mut w, T0, 2, DOWN);
        w.beat(T0 + 120, GOOD);
        let a = beats(&mut w, T0 + 180, 2, DOWN);
        assert!(a.is_empty(), "2 + good + 2 is not 3 in a row");
    }

    #[test]
    fn clearing_needs_three_good_beats_and_a_flap_does_not_page_again() {
        let mut w = Watchdog::new(T0);
        assert_eq!(pages(&beats(&mut w, T0, 3, DOWN)).len(), 1);
        beats(&mut w, T0 + 180, 2, GOOD);
        assert!(w.is_open(Component::Synapse), "two good beats do not clear");
        let a = beats(&mut w, T0 + 300, 3, DOWN);
        assert!(pages(&a).is_empty(), "still the same open alert: no storm");
        beats(&mut w, T0 + 480, 3, GOOD);
        assert!(!w.is_open(Component::Synapse), "three good beats clear");
        assert_eq!(
            pages(&beats(&mut w, T0 + 660, 3, DOWN)).len(),
            1,
            "a new fault after clearing pages again"
        );
    }

    #[test]
    fn the_pc_clears_after_three_consecutive_good_beats() {
        let mut w = Watchdog::new(T0);
        w.beat(T0, GOOD);
        assert_eq!(pages(&w.tick(T0 + 180)).len(), 1);
        w.beat(T0 + 600, GOOD);
        w.beat(T0 + 660, GOOD);
        assert!(w.is_open(Component::Pc));
        w.beat(T0 + 720, GOOD);
        assert!(!w.is_open(Component::Pc));
    }

    #[test]
    fn a_gap_in_the_pc_recovery_restarts_the_good_count() {
        let mut w = Watchdog::new(T0);
        w.beat(T0, GOOD);
        w.tick(T0 + 180);
        w.beat(T0 + 600, GOOD);
        w.beat(T0 + 660, GOOD);
        w.beat(T0 + 900, GOOD); // a long silence in between
        assert!(w.is_open(Component::Pc), "not three consecutive");
    }

    #[test]
    fn after_an_acknowledgement_there_is_no_page_for_thirty_minutes() {
        let mut w = Watchdog::new(T0);
        beats(&mut w, T0, 3, DOWN); // paged at T0+120
        w.acknowledged(Component::Synapse, T0 + 300);
        let t = T0 + 300;
        assert!(
            beats(&mut w, t + 60, 29, DOWN).is_empty(),
            "inside the quiet"
        );
        let a = w.beat(t + QUIET_AFTER_ACK, DOWN);
        assert_eq!(
            pages(&a).len(),
            1,
            "a persisting fault pages after the quiet"
        );
    }

    #[test]
    fn at_most_two_pages_an_hour_and_the_hold_is_audited_once() {
        let mut w = Watchdog::new(T0);
        beats(&mut w, T0, 3, DOWN); // page 1 at T0+120
        w.beat(T0, GOOD); // keep the PC alive in this test
        w.acknowledged(Component::Synapse, T0 + 130);
        // page 2: the PC goes silent
        let a = w.tick(T0 + 60 + 3 * BEAT_SECS);
        assert_eq!(pages(&a).len(), 1);
        // the quiet ends at T0+130+1800: a third page inside the hour is held
        let a = w.beat(T0 + 130 + QUIET_AFTER_ACK, DOWN);
        assert!(pages(&a).is_empty());
        assert_eq!(
            a,
            [Action::Suppressed {
                component: Component::Synapse,
                why: Why::HourlyCap
            }]
        );
        let again = w.beat(T0 + 130 + QUIET_AFTER_ACK + 60, DOWN);
        assert!(again.is_empty(), "audited once, not every beat");
        // the hour rolls: the oldest page is more than an hour old
        let a = w.beat(T0 + 120 + HOUR, DOWN);
        assert_eq!(pages(&a).len(), 1, "sent once the window has room");
    }

    #[test]
    fn a_pause_is_bounded_holds_pages_and_restarts_the_clock_when_it_ends() {
        let mut w = Watchdog::new(T0);
        assert_eq!(w.pause(0, T0), Err(PauseError::OutOfRange));
        assert_eq!(w.pause(61, T0), Err(PauseError::OutOfRange));
        w.beat(T0, GOOD);
        assert_eq!(w.pause(60, T0 + 30), Ok(T0 + 30 + 3600));
        assert!(
            w.tick(T0 + 1800).is_empty(),
            "a planned reboot is not an incident"
        );
        assert!(!w.is_open(Component::Pc));
        // the pause ends: the silence clock starts there, not at the last beat
        assert!(w.tick(T0 + 30 + 3600 + 2 * BEAT_SECS).is_empty());
        assert_eq!(pages(&w.tick(T0 + 30 + 3600 + 3 * BEAT_SECS)).len(), 1);
    }

    #[test]
    fn a_synapse_fault_during_a_pause_is_held_then_sent_if_it_persists() {
        let mut w = Watchdog::new(T0);
        w.pause(10, T0).unwrap();
        let a = beats(&mut w, T0, 3, DOWN);
        assert_eq!(
            a,
            [Action::Suppressed {
                component: Component::Synapse,
                why: Why::Paused
            }]
        );
        let a = w.beat(T0 + 600, DOWN);
        assert_eq!(
            pages(&a).len(),
            1,
            "the pause is over and the fault remains"
        );
    }
}
