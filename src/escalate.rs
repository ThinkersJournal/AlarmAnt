//! A5 timer driver: runs the escalation state machine on a fixed timer.
//!
//! The caller owns the clock (seconds) and calls [`Escalator::tick`]; nothing
//! here sleeps, sends over a network or persists. Rules from `DESIGN-A5.md`:
//! one setting `N` minutes, the same timer for every step, no reordering, no
//! timer after acknowledgement, no third tier. The phone reports only
//! Acknowledge: there is no received/viewed/decline input.

use std::collections::BTreeMap;

use crate::{Alert, AlertChannel, AlertEvent, AlertState, Contact, Outcome, Stage};

/// Default `N`, in minutes.
pub const DEFAULT_MINUTES: u64 = 5;

/// What is stored for one open alert. Person 2's number is inside the
/// [`Contact`], which never prints it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EscalationRecord {
    pub alert_id: String,
    pub person2: Contact,
    pub state: AlertState,
    pub stage: Stage,
    /// Unix seconds of the next timeout; `None` once unresolved.
    pub due: Option<u64>,
}

/// Durable escalation state (a PersistAnt-backed implementation comes with the
/// service). Write-through: the [`Escalator`] saves on every change and
/// removes a record when the alert is acknowledged.
pub trait EscalationStore {
    fn save(&self, record: EscalationRecord);
    fn remove(&self, alert_id: &str);
    fn load_all(&self) -> Vec<EscalationRecord>;
}

/// In-memory [`EscalationStore`], for tests and until PersistAnt is wired.
#[derive(Debug, Default)]
pub struct MemoryEscalationStore {
    records: std::sync::Mutex<BTreeMap<String, EscalationRecord>>,
}

impl MemoryEscalationStore {
    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, EscalationRecord>> {
        self.records.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl EscalationStore for MemoryEscalationStore {
    fn save(&self, record: EscalationRecord) {
        self.lock().insert(record.alert_id.clone(), record);
    }
    fn remove(&self, alert_id: &str) {
        self.lock().remove(alert_id);
    }
    fn load_all(&self) -> Vec<EscalationRecord> {
        self.lock().values().cloned().collect()
    }
}

impl<S: EscalationStore + ?Sized> EscalationStore for std::sync::Arc<S> {
    fn save(&self, record: EscalationRecord) {
        (**self).save(record)
    }
    fn remove(&self, alert_id: &str) {
        (**self).remove(alert_id)
    }
    fn load_all(&self) -> Vec<EscalationRecord> {
        (**self).load_all()
    }
}

fn record_of(entry: &Entry) -> EscalationRecord {
    EscalationRecord {
        alert_id: entry.alert.id().to_string(),
        person2: entry.alert.person2().clone(),
        state: entry.alert.state(),
        stage: entry.alert.stage(),
        due: entry.due,
    }
}

struct Entry {
    alert: Alert,
    /// When the next timeout fires; `None` once unresolved (only an ack ends it).
    due: Option<u64>,
}

/// Escalation timers for every open alert.
pub struct Escalator {
    timeout_secs: u64,
    open: BTreeMap<String, Entry>,
    store: Option<Box<dyn EscalationStore>>,
}

impl Escalator {
    /// `minutes` is the single setting `N`; 0 is raised to 1.
    pub fn new(minutes: u64) -> Self {
        Escalator {
            timeout_secs: minutes.max(1) * 60,
            open: BTreeMap::new(),
            store: None,
        }
    }

    /// Like [`Escalator::new`], but resumes every open alert found in `store`
    /// and writes every later change through to it. A timer that came due
    /// while the service was down fires on the next tick, one step.
    pub fn with_store(minutes: u64, store: Box<dyn EscalationStore>) -> Self {
        let mut e = Escalator::new(minutes);
        for r in store.load_all() {
            if r.state == AlertState::Acked {
                store.remove(&r.alert_id);
                continue;
            }
            e.open.insert(
                r.alert_id.clone(),
                Entry {
                    alert: Alert::restore(r.alert_id, r.person2, r.state, r.stage),
                    due: r.due,
                },
            );
        }
        e.store = Some(store);
        e
    }

    /// Start the timer for an alert already sent to person 1 at `now`.
    /// Returns false (and changes nothing) if `alert_id` is already open.
    pub fn register(&mut self, alert_id: &str, person2: Contact, now: u64) -> bool {
        if self.open.contains_key(alert_id) {
            return false;
        }
        let entry = Entry {
            alert: Alert::new(alert_id, person2),
            due: Some(now + self.timeout_secs),
        };
        if let Some(store) = &self.store {
            store.save(record_of(&entry));
        }
        self.open.insert(alert_id.to_string(), entry);
        true
    }

    /// An acknowledgement from anyone. Cancels the timer. `None` if the alert
    /// is not open here (unknown, or already acknowledged).
    pub fn ack(&mut self, alert_id: &str, channel: &dyn AlertChannel) -> Option<Outcome> {
        let mut entry = self.open.remove(alert_id)?;
        if let Some(store) = &self.store {
            store.remove(alert_id);
        }
        Some(entry.alert.on(AlertEvent::Ack, channel))
    }

    /// Fire every timeout due at `now`, once each, and restart that alert's
    /// timer at `now + N` (a late tick never fires two steps at once).
    /// Results are in alert-id order.
    pub fn tick(&mut self, now: u64, channel: &dyn AlertChannel) -> Vec<(String, Outcome)> {
        let mut fired = Vec::new();
        let store = self.store.as_deref();
        for (id, entry) in self.open.iter_mut() {
            if entry.due.is_some_and(|d| d <= now) {
                let out = entry.alert.on(AlertEvent::TimeoutElapsed, channel);
                entry.due = match entry.alert.state() {
                    AlertState::Unresolved => None,
                    _ => Some(now + self.timeout_secs),
                };
                if let Some(store) = store {
                    store.save(record_of(entry));
                }
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
    fn restart_resumes_the_chain_from_the_store() {
        let store = std::sync::Arc::new(MemoryEscalationStore::default());
        let ch = Fake::default();
        let mut e = Escalator::with_store(5, Box::new(store.clone()));
        e.register("a", contact(), 0);
        e.register("b", contact(), 0);
        e.tick(300, &ch); // both texted
        e.ack("b", &ch);
        drop(e); // restart
        assert_eq!(store.load_all().len(), 1, "acked alert is gone");
        let mut e = Escalator::with_store(5, Box::new(store.clone()));
        assert_eq!(e.state("a"), Some(AlertState::Escalated));
        assert_eq!(e.state("b"), None);
        assert_eq!(e.next_due(), Some(600));
        // Down for an hour: one step on the next tick (the call), not unresolved.
        e.tick(4000, &ch);
        assert_eq!(*ch.0.borrow(), vec!["sms a", "sms b", "voice a"]);
        assert_eq!(e.state("a"), Some(AlertState::Escalated));
    }

    #[test]
    fn unresolved_survives_restart_without_a_timer_and_ack_removes_it() {
        let store = std::sync::Arc::new(MemoryEscalationStore::default());
        let ch = Fake::default();
        let mut e = Escalator::with_store(1, Box::new(store.clone()));
        e.register("a", contact(), 0);
        for t in [60, 120, 180] {
            e.tick(t, &ch);
        }
        drop(e);
        let mut e = Escalator::with_store(1, Box::new(store.clone()));
        assert_eq!(e.state("a"), Some(AlertState::Unresolved));
        assert_eq!(e.next_due(), None);
        assert!(e.tick(9999, &ch).is_empty());
        assert!(e.ack("a", &ch).is_some());
        assert!(store.load_all().is_empty());
    }

    #[test]
    fn stored_record_never_prints_the_number() {
        let store = std::sync::Arc::new(MemoryEscalationStore::default());
        let mut e = Escalator::with_store(5, Box::new(store.clone()));
        e.register("a", contact(), 0);
        let saved = store.load_all();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].person2.number(), "+15550100");
        assert!(!format!("{saved:?}").contains("5550100"));
    }

    fn store_with(records: Vec<EscalationRecord>) -> std::sync::Arc<MemoryEscalationStore> {
        let s = std::sync::Arc::new(MemoryEscalationStore::default());
        for r in records {
            s.save(r);
        }
        s
    }

    fn rec(id: &str, state: AlertState, stage: Stage, due: Option<u64>) -> EscalationRecord {
        EscalationRecord {
            alert_id: id.into(),
            person2: contact(),
            state,
            stage,
            due,
        }
    }

    #[test]
    fn resuming_sends_nothing_by_itself() {
        let s = store_with(vec![rec("a", AlertState::Sent, Stage::First, Some(1))]);
        let ch = Fake::default();
        let _e = Escalator::with_store(5, Box::new(s));
        assert!(ch.0.borrow().is_empty());
    }

    #[test]
    fn every_overdue_alert_fires_exactly_one_step_after_a_long_outage() {
        let s = store_with(vec![
            rec("a", AlertState::Sent, Stage::First, Some(10)),
            rec("b", AlertState::Escalated, Stage::Sms, Some(20)),
            rec("c", AlertState::Escalated, Stage::Voice, Some(30)),
        ]);
        let (mut e, ch) = (
            Escalator::with_store(5, Box::new(s.clone())),
            Fake::default(),
        );
        let fired = e.tick(1_000_000, &ch);
        assert_eq!(fired.len(), 3);
        assert_eq!(*ch.0.borrow(), vec!["sms a", "voice b"]);
        assert_eq!(e.state("a"), Some(AlertState::Escalated));
        assert_eq!(e.state("b"), Some(AlertState::Escalated));
        assert_eq!(e.state("c"), Some(AlertState::Unresolved));
        // Same tick again: nothing further, each timer restarted at now + N.
        assert!(e.tick(1_000_000, &ch).is_empty());
        assert_eq!(e.next_due(), Some(1_000_300));
    }

    #[test]
    fn stage_and_due_are_saved_after_each_step() {
        let s = store_with(vec![]);
        let (mut e, ch) = (
            Escalator::with_store(5, Box::new(s.clone())),
            Fake::default(),
        );
        e.register("a", contact(), 0);
        let saved = |s: &MemoryEscalationStore| s.load_all().remove(0);
        assert_eq!((saved(&s).stage, saved(&s).due), (Stage::First, Some(300)));
        e.tick(300, &ch);
        assert_eq!((saved(&s).stage, saved(&s).due), (Stage::Sms, Some(600)));
        e.tick(600, &ch);
        assert_eq!((saved(&s).stage, saved(&s).due), (Stage::Voice, Some(900)));
        e.tick(900, &ch);
        let r = saved(&s);
        assert_eq!((r.state, r.due), (AlertState::Unresolved, None));
    }

    #[test]
    fn restart_after_the_call_ends_unresolved_without_a_second_call() {
        let s = store_with(vec![rec(
            "a",
            AlertState::Escalated,
            Stage::Voice,
            Some(900),
        )]);
        let (mut e, ch) = (Escalator::with_store(5, Box::new(s)), Fake::default());
        let fired = e.tick(900, &ch);
        assert_eq!(fired[0].1.audit, vec![AuditEvent::Unresolved]);
        assert!(ch.0.borrow().is_empty(), "no second call, no new text");
    }

    #[test]
    fn a_stored_acked_record_is_dropped_on_load() {
        let s = store_with(vec![rec("a", AlertState::Acked, Stage::Sms, None)]);
        let (mut e, ch) = (
            Escalator::with_store(5, Box::new(s.clone())),
            Fake::default(),
        );
        assert_eq!(e.state("a"), None);
        assert!(s.load_all().is_empty());
        assert!(e.tick(99_999, &ch).is_empty());
    }

    #[test]
    fn not_yet_due_after_restart_waits_for_its_own_time() {
        let s = store_with(vec![rec("a", AlertState::Sent, Stage::First, Some(500))]);
        let (mut e, ch) = (Escalator::with_store(5, Box::new(s)), Fake::default());
        assert!(e.tick(499, &ch).is_empty());
        assert_eq!(e.tick(500, &ch).len(), 1);
    }

    #[test]
    fn zero_minutes_is_raised_to_one() {
        let (mut e, ch) = (Escalator::new(0), Fake::default());
        e.register("a", contact(), 0);
        assert!(e.tick(59, &ch).is_empty());
        assert_eq!(e.tick(60, &ch).len(), 1);
    }
}
