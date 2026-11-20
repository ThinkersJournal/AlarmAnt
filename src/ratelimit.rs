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
    owners: Mutex<HashMap<String, VecDeque<u64>>>,
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
        let mut owners = self.owners.lock().unwrap_or_else(|p| p.into_inner());
        let times = owners.entry(owner.to_owned()).or_default();
        while times.front().is_some_and(|t| now.saturating_sub(*t) >= DAY) {
            times.pop_front();
        }
        let in_hour = times
            .iter()
            .filter(|t| now.saturating_sub(**t) < HOUR)
            .count();
        if in_hour >= self.limits.per_hour as usize || times.len() >= self.limits.per_day as usize {
            return false;
        }
        times.push_back(now);
        true
    }
}

impl Default for AlertLimiter {
    fn default() -> Self {
        AlertLimiter::new(RateLimits::default())
    }
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
    fn a_clock_that_goes_back_never_panics_or_frees_the_budget() {
        let l = lim(1, 100);
        assert!(l.allow("a", 5000));
        assert!(
            !l.allow("a", 100),
            "an earlier time is still inside the window"
        );
    }
}
