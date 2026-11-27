// SPDX-License-Identifier: MIT OR Apache-2.0
//! Observer B of the watchdog (docs/SYNAPSE-ADAPTER.md 5): the PC checks the phone server. If
//! `GET /health` fails 3 checks in a row (one a minute, a fixed cadence), the PC sounds its own siren
//! and writes a terminal notice; 3 good checks in a row end it. The PC cannot reach the phone
//! without the server, so this is a local alarm only.
//!
//! Binding limits (board 490 C; PATENT-SWEEP S4, S6, S7): fixed counts, no learning, no score;
//! nothing here is sent to the server, is a presence probe of the phone, or feeds escalation. The
//! check sends no key and the server records nothing about it.

use crate::pcclient::Siren;

/// Seconds between health checks (fixed, not negotiated).
pub const CHECK_SECS: u64 = 60;
/// Consecutive failed checks that sound the siren, and good ones that end it.
pub const THRESHOLD: u32 = 3;
/// The fixed text `GET /health` answers.
pub const HEALTH_TEXT: &str = "ok";

/// What a check result changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notice {
    /// The server stopped answering: sound the siren, tell the terminal.
    ServerDown,
    /// The server answers again: stop the siren.
    ServerUp,
}

/// The counting, free of any siren.
#[derive(Debug, Default, Clone)]
pub struct ServerWatch {
    bad: u32,
    good: u32,
    down: bool,
}

impl ServerWatch {
    /// One check result (`true` = the server answered the fixed text).
    pub fn observe(&mut self, ok: bool) -> Option<Notice> {
        if ok {
            self.bad = 0;
            if self.down {
                self.good += 1;
                if self.good >= THRESHOLD {
                    self.down = false;
                    self.good = 0;
                    return Some(Notice::ServerUp);
                }
            }
        } else {
            self.good = 0;
            self.bad += 1;
            if self.bad >= THRESHOLD && !self.down {
                self.down = true;
                return Some(Notice::ServerDown);
            }
        }
        None
    }

    pub fn is_down(&self) -> bool {
        self.down
    }
}

/// [`ServerWatch`] driving a siren. The operator's Stop silences it; it sounds again only after the
/// server has recovered and failed again (one alarm per outage, so a dismissed one stays dismissed).
pub struct HealthWatcher<S: Siren> {
    watch: ServerWatch,
    siren: S,
    sounding: bool,
}

impl<S: Siren> HealthWatcher<S> {
    pub fn new(siren: S) -> Self {
        HealthWatcher {
            watch: ServerWatch::default(),
            siren,
            sounding: false,
        }
    }

    pub fn is_sounding(&self) -> bool {
        self.sounding
    }

    /// Applies one check result; returns the change, for the terminal notice.
    pub fn observe(&mut self, ok: bool) -> Option<Notice> {
        let n = self.watch.observe(ok);
        match n {
            Some(Notice::ServerDown) => {
                self.siren.start();
                self.sounding = true;
            }
            Some(Notice::ServerUp) if self.sounding => {
                self.siren.stop();
                self.sounding = false;
            }
            _ => {
                if self.sounding {
                    self.siren.keep();
                }
            }
        }
        n
    }

    /// The operator chose to stop.
    pub fn operator_stop(&mut self) {
        if self.sounding {
            self.siren.stop();
            self.sounding = false;
        }
    }
}

/// The sentence for the terminal.
pub fn terminal_notice(n: Notice) -> &'static str {
    match n {
        Notice::ServerDown => {
            "the AlarmAnt server has not answered 3 checks in a row; phone alerts cannot be sent"
        }
        Notice::ServerUp => "the AlarmAnt server answers again",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Log(Vec<&'static str>);
    impl Siren for Log {
        fn start(&mut self) {
            self.0.push("start");
        }
        fn keep(&mut self) {
            self.0.push("keep");
        }
        fn stop(&mut self) {
            self.0.push("stop");
        }
    }

    fn run(w: &mut HealthWatcher<Log>, oks: &[bool]) -> Vec<Option<Notice>> {
        oks.iter().map(|&o| w.observe(o)).collect()
    }

    #[test]
    fn two_failed_checks_do_nothing_and_the_third_sounds() {
        let mut w = HealthWatcher::new(Log::default());
        assert_eq!(run(&mut w, &[false, false]), [None, None]);
        assert!(!w.is_sounding());
        assert_eq!(w.observe(false), Some(Notice::ServerDown));
        assert!(w.is_sounding());
        assert_eq!(w.siren.0, ["start"]);
    }

    #[test]
    fn a_good_check_between_failures_resets_the_count() {
        let mut w = HealthWatcher::new(Log::default());
        let r = run(&mut w, &[false, false, true, false, false]);
        assert!(r.iter().all(Option::is_none));
        assert!(w.siren.0.is_empty());
    }

    #[test]
    fn it_ends_after_three_good_checks_and_not_before() {
        let mut w = HealthWatcher::new(Log::default());
        run(&mut w, &[false, false, false]);
        assert_eq!(run(&mut w, &[true, true]), [None, None]);
        assert!(w.is_sounding(), "two good checks do not end it");
        assert_eq!(run(&mut w, &[false, true, true]), [None, None, None]);
        assert!(
            w.is_sounding(),
            "a failure in between restarts the good count"
        );
        assert_eq!(w.observe(true), Some(Notice::ServerUp));
        assert!(!w.is_sounding());
        assert_eq!(w.siren.0.last(), Some(&"stop"));
    }

    #[test]
    fn a_dismissed_alarm_stays_dismissed_until_a_new_outage() {
        let mut w = HealthWatcher::new(Log::default());
        run(&mut w, &[false, false, false]);
        w.operator_stop();
        assert!(!w.is_sounding());
        run(&mut w, &[false, false, false, false]);
        assert!(!w.is_sounding(), "the same outage does not re-sound");
        run(&mut w, &[true, true, true]);
        run(&mut w, &[false, false, false]);
        assert!(w.is_sounding(), "a new outage sounds again");
    }

    #[test]
    fn the_notice_text_is_fixed_and_says_nothing_else() {
        assert!(terminal_notice(Notice::ServerDown).contains("has not answered"));
        assert!(terminal_notice(Notice::ServerUp).contains("answers again"));
    }
}
