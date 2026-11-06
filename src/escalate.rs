//! A5 timer driver: runs the escalation state machine on a fixed timer.
//!
//! The caller owns the clock (seconds) and calls [`Escalator::tick`]; nothing
//! here sleeps, sends over a network or persists. Rules from `DESIGN-A5.md`:
//! one setting `N` minutes, the same timer for every step, no reordering, no
//! timer after acknowledgement, no third tier. The phone reports only
//! Acknowledge: there is no received/viewed/decline input.

use std::collections::BTreeMap;

use crate::{Alert, AlertChannel, AlertEvent, AlertState, Contact, Outcome};

/// Default `N`, in minutes.
pub const DEFAULT_MINUTES: u64 = 5;

struct Entry {
    alert: Alert,
    /// When the next timeout fires; `None` once unresolved (only an ack ends it).
    due: Option<u64>,
}

/// Escalation timers for every open alert.
pub struct Escalator {
    timeout_secs: u64,
    open: BTreeMap<String, Entry>,
}

impl Escalator {
    /// `minutes` is the single setting `N`; 0 is raised to 1.
    pub fn new(minutes: u64) -> Self {
        Escalator {
            timeout_secs: minutes.max(1) * 60,
            open: BTreeMap::new(),
        }
    }

    /// Start the timer for an alert already sent to person 1 at `now`.
    /// Returns false (and changes nothing) if `alert_id` is already open.
    pub fn register(&mut self, alert_id: &str, person2: Contact, now: u64) -> bool {
        if self.open.contains_key(alert_id) {
            return false;
        }
        self.open.insert(
            alert_id.to_string(),
            Entry {
                alert: Alert::new(alert_id, person2),
                due: Some(now + self.timeout_secs),
            },
        );
        true
    }

    /// An acknowledgement from anyone. Cancels the timer. `None` if the alert
    /// is not open here (unknown, or already acknowledged).
    pub fn ack(&mut self, alert_id: &str, channel: &dyn AlertChannel) -> Option<Outcome> {
        let mut entry = self.open.remove(alert_id)?;
        Some(entry.alert.on(AlertEvent::Ack, channel))
    }

    /// Fire every timeout due at `now`, once each, and restart that alert's
    /// timer at `now + N` (a late tick never fires two steps at once).
    /// Results are in alert-id order.
    pub fn tick(&mut self, now: u64, channel: &dyn AlertChannel) -> Vec<(String, Outcome)> {
        let mut fired = Vec::new();
        for (id, entry) in self.open.iter_mut() {
            if entry.due.is_some_and(|d| d <= now) {
                let out = entry.alert.on(AlertEvent::TimeoutElapsed, channel);
                entry.due = match entry.alert.state() {
                    AlertState::Unresolved => None,
                    _ => Some(now + self.timeout_secs),
                };
                fired.push((id.clone(), out));
            }
        }
        fired
    }

    /// Where an open alert stands; `None` if acknowledged or unknown.
    pub fn state(&self, alert_id: &str) -> Option<AlertState> {
        self.open.get(alert_id).map(|e| e.alert.state())
    }

    /// The earliest time something fires, for the caller's sleep.
    pub fn next_due(&self) -> Option<u64> {
        self.open.values().filter_map(|e| e.due).min()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AuditEvent, ChannelError};
    use std::cell::RefCell;

    #[derive(Default)]
    struct Fake(RefCell<Vec<String>>);

    impl AlertChannel for Fake {
        fn sms(&self, _: &Contact, id: &str) -> Result<(), ChannelError> {
            self.0.borrow_mut().push(format!("sms {id}"));
            Ok(())
        }
        fn voice(&self, _: &Contact, id: &str) -> Result<(), ChannelError> {
            self.0.borrow_mut().push(format!("voice {id}"));
            Ok(())
        }
    }

    fn contact() -> Contact {
        Contact::new("+15550100")
    }

    #[test]
    fn nothing_fires_before_n_minutes() {
        let (mut e, ch) = (Escalator::new(5), Fake::default());
        e.register("a", contact(), 1000);
        assert!(e.tick(1000 + 299, &ch).is_empty());
        assert!(ch.0.borrow().is_empty());
        assert_eq!(e.next_due(), Some(1300));
    }

    #[test]
    fn full_chain_on_a_fixed_timer() {
        let (mut e, ch) = (Escalator::new(5), Fake::default());
        e.register("a", contact(), 0);
        let r = e.tick(300, &ch);
        assert_eq!(r[0].1.audit, vec![AuditEvent::Escalated]);
        assert_eq!(e.state("a"), Some(AlertState::Escalated));
        assert!(e.tick(599, &ch).is_empty());
        e.tick(600, &ch);
        let r = e.tick(900, &ch);
        assert_eq!(r[0].1.audit, vec![AuditEvent::Unresolved]);
        assert_eq!(e.state("a"), Some(AlertState::Unresolved));
        assert_eq!(*ch.0.borrow(), vec!["sms a", "voice a"]);
        assert_eq!(e.next_due(), None, "no timer once unresolved");
        assert!(e.tick(99_999, &ch).is_empty(), "no third tier");
    }

    #[test]
    fn late_tick_fires_one_step_only() {
        let (mut e, ch) = (Escalator::new(5), Fake::default());
        e.register("a", contact(), 0);
        e.tick(10_000, &ch);
        assert_eq!(*ch.0.borrow(), vec!["sms a"]);
        assert_eq!(e.next_due(), Some(10_300));
    }

    #[test]
    fn ack_cancels_the_timer() {
        let (mut e, ch) = (Escalator::new(5), Fake::default());
        e.register("a", contact(), 0);
        let out = e.ack("a", &ch).unwrap();
        assert_eq!(out.audit, vec![AuditEvent::Acked]);
        assert_eq!(e.state("a"), None);
        assert_eq!(e.next_due(), None);
        assert!(e.tick(10_000, &ch).is_empty());
        assert!(ch.0.borrow().is_empty());
        assert!(e.ack("a", &ch).is_none(), "second ack is a no-op");
    }

    #[test]
    fn ack_ends_an_unresolved_alert() {
        let (mut e, ch) = (Escalator::new(1), Fake::default());
        e.register("a", contact(), 0);
        for t in [60, 120, 180] {
            e.tick(t, &ch);
        }
        assert_eq!(e.state("a"), Some(AlertState::Unresolved));
        assert!(e.ack("a", &ch).is_some());
        assert_eq!(e.state("a"), None);
    }

    #[test]
    fn alerts_are_independent_and_duplicates_refused() {
        let (mut e, ch) = (Escalator::new(5), Fake::default());
        assert!(e.register("a", contact(), 0));
        assert!(e.register("b", contact(), 100));
        assert!(!e.register("a", contact(), 200), "duplicate id");
        let r = e.tick(300, &ch);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].0, "a");
        assert_eq!(e.state("b"), Some(AlertState::Sent));
        assert_eq!(e.next_due(), Some(400));
    }

    #[test]
    fn zero_minutes_is_raised_to_one() {
        let (mut e, ch) = (Escalator::new(0), Fake::default());
        e.register("a", contact(), 0);
        assert!(e.tick(59, &ch).is_empty());
        assert_eq!(e.tick(60, &ch).len(), 1);
    }
}
