//! ROADMAP A10: a rate limit on `POST /alert`, keyed by the key's owner.
//!
//! The limit is on **raised alerts**, per owner, over a rolling hour and a rolling day. A request with a wrong
//! key never reaches it (it is a 401 first) and spends nothing; a refused request is not counted, so being
//! refused does not extend the wait. Owners, not keys: today there is one API key, so one owner, but every
//! key of one sender must name the same owner so that a second key cannot double the budget.
//!
//! In memory: a restart forgets the counts, which can only let a few more alerts through straight after one.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

/// The caps; both settable (`ALARMANT_RATE_PER_HOUR`, `ALARMANT_RATE_PER_DAY`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimits {
    pub per_hour: u32,
    pub per_day: u32,
}

impl Default for RateLimits {
    fn default() -> Self {
        RateLimits {
            per_hour: 6,
            per_day: 20,
        }
    }
}

/// Counts raised alerts per owner.
pub struct AlertLimiter {
    limits: RateLimits,
    owners: Mutex<HashMap<String, Owner>>,
}

#[derive(Default)]
struct Owner {
    /// When each counted alert was raised, oldest first.
    times: VecDeque<u64>,
    /// Alerts refused since the last one that was allowed.
    dropped: u32,
    /// When the human was last told that this owner is being throttled.
    last_notice: Option<u64>,
}

/// What the limiter decided about one alert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Allowed,
    /// Refused. A refusal is never silent: `dropped` counts the alerts lost since the last allowed one, and
    /// `notice_due` is true for at most one refusal an hour per owner, the one the human is paged about.
    /// `clears_at` is when the owner's budget next has room (unix seconds).
    Refused {
        dropped: u32,
        notice_due: bool,
        clears_at: u64,
    },
}

/// The owner every key belongs to until keys carry owners of their own (one API key exists today).
pub const DEFAULT_OWNER: &str = "api";

impl AlertLimiter {
    pub fn new(limits: RateLimits) -> Self {
        AlertLimiter {
            limits,
            owners: Mutex::new(HashMap::new()),
        }
    }

    /// Whether `owner` may raise an alert at `now` (unix seconds); when it may, the alert is counted.
    pub fn allow(&self, owner: &str, now: u64) -> bool {
        self.check(owner, now) == Decision::Allowed
    }

    /// The decision for one alert, with what a refusal needs to be reported (see [`Decision`]).
    pub fn check(&self, owner: &str, now: u64) -> Decision {
        let mut owners = self.owners.lock().unwrap_or_else(|p| p.into_inner());
        let o = owners.entry(owner.to_owned()).or_default();
        while o
            .times
            .front()
            .is_some_and(|t| now.saturating_sub(*t) >= DAY)
        {
            o.times.pop_front();
        }
        let in_hour: Vec<u64> = o
            .times
            .iter()
            .copied()
            .filter(|t| now.saturating_sub(*t) < HOUR)
            .collect();
        let hour_full = in_hour.len() >= self.limits.per_hour as usize;
        let day_full = o.times.len() >= self.limits.per_day as usize;
        if !hour_full && !day_full {
            o.times.push_back(now);
            o.dropped = 0;
            return Decision::Allowed;
        }
        // The budget has room again when the alert that fills the binding window ages out of it.
        let hour_clears = in_hour
            .get(in_hour.len().saturating_sub(self.limits.per_hour as usize))
            .map_or(now, |t| t + HOUR);
        let day_clears = o
            .times
            .get(o.times.len().saturating_sub(self.limits.per_day as usize))
            .map_or(now, |t| t + DAY);
        let clears_at = match (hour_full, day_full) {
            (true, true) => hour_clears.max(day_clears),
            (true, false) => hour_clears,
            _ => day_clears,
        };
        o.dropped = o.dropped.saturating_add(1);
        let notice_due = o.last_notice.is_none_or(|t| now.saturating_sub(t) >= HOUR);
        if notice_due {
            o.last_notice = Some(now);
        }
        Decision::Refused {
            dropped: o.dropped,
            notice_due,
            clears_at,
        }
    }
}

impl Default for AlertLimiter {
    fn default() -> Self {
        AlertLimiter::new(RateLimits::default())
    }
}

/// `HH:MM UTC` of a unix time, for a human-readable sentence (the date is omitted: it is at most a day away).
pub fn utc_hhmm(unix: u64) -> String {
    let s = unix % DAY;
    format!("{:02}:{:02} UTC", s / 3600, (s % 3600) / 60)
}

const HOUR: u64 = 3600;
const DAY: u64 = 86_400;

#[cfg(test)]
mod tests {
    use super::*;

    fn lim(h: u32, d: u32) -> AlertLimiter {
        AlertLimiter::new(RateLimits {
            per_hour: h,
            per_day: d,
        })
    }

    #[test]
    fn the_n_plus_one_th_in_the_hour_is_refused_and_the_window_rolls() {
        let l = lim(3, 100);
        for i in 0..3 {
            assert!(l.allow("a", 1000 + i), "{i}");
        }
        assert!(!l.allow("a", 1010), "the 4th inside the hour");
        assert!(
            !l.allow("a", 1000 + 3599),
            "still inside the hour of the first"
        );
        assert!(
            l.allow("a", 1000 + 3600 + 3),
            "the hour has rolled past all three"
        );
    }

    #[test]
    fn a_refused_alert_is_not_counted_so_it_does_not_extend_the_wait() {
        let l = lim(1, 100);
        assert!(l.allow("a", 0));
        for t in [10, 20, 30, 3599] {
            assert!(!l.allow("a", t));
        }
        assert!(
            l.allow("a", 3600),
            "the wait ended one hour after the ONE counted alert"
        );
    }

    #[test]
    fn the_day_cap_holds_even_when_each_hour_is_inside_its_cap() {
        let l = lim(100, 4);
        for i in 0..4 {
            assert!(l.allow("a", i * 4000));
        }
        assert!(!l.allow("a", 4 * 4000), "the 5th inside the day");
        assert!(
            l.allow("a", 86_400 + 1),
            "the day has rolled past the first"
        );
    }

    #[test]
    fn owners_have_separate_budgets_and_the_same_owner_shares_one() {
        let l = lim(2, 100);
        assert!(l.allow("alice", 0) && l.allow("alice", 1));
        assert!(!l.allow("alice", 2));
        // keys of one sender name the same owner, so a second key cannot double the budget
        assert!(!l.allow("alice", 3));
        assert!(l.allow("bob", 3), "another owner is unaffected");
    }

    #[test]
    fn a_refusal_is_never_silent_it_counts_and_pages_once_an_hour() {
        let l = lim(1, 100);
        assert_eq!(l.check("a", 1000), Decision::Allowed);
        let first = l.check("a", 1100);
        assert_eq!(
            first,
            Decision::Refused {
                dropped: 1,
                notice_due: true,
                clears_at: 1000 + 3600
            }
        );
        for (i, t) in [1200u64, 1300, 1400].iter().enumerate() {
            match l.check("a", *t) {
                Decision::Refused {
                    dropped,
                    notice_due,
                    clears_at,
                } => {
                    assert_eq!(dropped, i as u32 + 2, "the count keeps rising");
                    assert!(!notice_due, "the human is paged once, not per refusal");
                    assert_eq!(clears_at, 4600);
                }
                other => panic!("{other:?}"),
            }
        }
        // an alert allowed again resets the count
        assert_eq!(l.check("a", 4700), Decision::Allowed);
        match l.check("a", 4800) {
            Decision::Refused {
                dropped,
                notice_due,
                ..
            } => {
                assert_eq!(dropped, 1);
                assert!(
                    notice_due,
                    "3700 s after the last page is past the hour: paged again"
                );
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_clear_time_reads_as_utc_hours_and_minutes() {
        assert_eq!(utc_hhmm(0), "00:00 UTC");
        assert_eq!(utc_hhmm(86_400 + 13 * 3600 + 5 * 60 + 59), "13:05 UTC");
    }

    #[test]
    fn a_clock_that_goes_back_never_panics_or_frees_the_budget() {
        let l = lim(1, 100);
        assert!(l.allow("a", 5000));
        assert!(
            !l.allow("a", 100),
            "an earlier time is still inside the window"
        );
    }
}
