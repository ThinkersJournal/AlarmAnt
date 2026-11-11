//! A6: the daily self-test alarm.
//!
//! Once a day the sender raises a real alert through the same path as any other (A9, FCM, the
//! phone's alarm). The person acknowledges it. An alert still unacknowledged by the deadline, or
//! one the sender no longer knows, is the failure signal: the chain is broken somewhere between the
//! push service and the person. The caller owns the clock and calls [`SelfTest::tick`] regularly;
//! where a failure is reported to (not the phone, which may be the broken part) is the caller's.

use crate::AlertState;
use crate::ack::AlertStore;
use crate::fcm::PushTransport;
use crate::raise::{DeviceRegistry, RaiseError, raise_authorised};

const DAY: u64 = 86_400;

/// Title of the test alert. The body tells the person what to do.
pub const TITLE: &str = "AlarmAnt self-test";
pub const BODY: &str = "Daily test of the alert chain. Acknowledge it.";

/// How the previous test ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Acknowledged: the chain works.
    Passed,
    /// Not acknowledged by the deadline: the push, the alarm or the person failed.
    NotAcked,
    /// The sender has no record of it (store lost or wrong): the ack could never have landed.
    Lost,
}

impl Verdict {
    pub fn is_failure(self) -> bool {
        self != Verdict::Passed
    }
}

/// The latest scheduled run time at or before `now`: `at_secs` seconds after midnight UTC.
pub fn latest_slot(now: u64, at_secs: u64) -> u64 {
    let slot = now - now % DAY + at_secs % DAY;
    if slot > now {
        slot.saturating_sub(DAY)
    } else {
        slot
    }
}

/// Where the self-test stands. Persist it (it is two small values) to survive a restart.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SelfTest {
    /// The slot of the last test successfully raised.
    pub last_slot: Option<u64>,
    /// The alert id of that test.
    pub alert_id: Option<String>,
    /// Whether its verdict has been returned already.
    pub reported: bool,
}

/// What one [`SelfTest::tick`] did.
#[derive(Debug, PartialEq, Eq)]
pub struct Tick {
    /// The previous test's final verdict, returned once.
    pub verdict: Option<Verdict>,
    /// Set when a test was due: the new alert id, or why it could not be raised. A failed raise is
    /// itself a failure signal and is retried on the next tick.
    pub raised: Option<Result<String, RaiseError>>,
}

impl SelfTest {
    fn verdict(&self, store: &dyn AlertStore, now: u64, deadline_secs: u64) -> Option<Verdict> {
        if self.reported {
            return None;
        }
        let id = self.alert_id.as_deref()?;
        let Some(r) = store.get(id) else {
            return Some(Verdict::Lost);
        };
        if r.state == AlertState::Acked {
            Some(Verdict::Passed)
        } else if now.saturating_sub(r.sent_at) >= deadline_secs {
            Some(Verdict::NotAcked)
        } else {
            None
        }
    }

    /// Reports the previous test if it has ended, and raises today's test if one is due.
    /// `at_secs` is the time of day (seconds after midnight UTC); `deadline_secs` how long a test
    /// may stay unacknowledged before it counts as failed.
    pub async fn tick(
        &mut self,
        devices: &dyn DeviceRegistry,
        transport: &impl PushTransport,
        store: &dyn AlertStore,
        now: u64,
        at_secs: u64,
        deadline_secs: u64,
    ) -> Tick {
        let verdict = self.verdict(store, now, deadline_secs);
        if verdict.is_some() {
            self.reported = true;
        }
        let slot = latest_slot(now, at_secs);
        let mut raised = None;
        if self.last_slot.is_none_or(|last| slot > last) {
            let body = serde_json::json!({ "title": TITLE, "body": BODY }).to_string();
            let r = raise_authorised(&body, devices, transport, store, now).await;
            if let Ok(id) = &r {
                self.last_slot = Some(slot);
                self.alert_id = Some(id.clone());
                self.reported = false;
            }
            raised = Some(r);
        }
        Tick { verdict, raised }
    }

    /// Serialises the state for a durable store (a small JSON object).
    pub fn to_json(&self) -> String {
        serde_json::json!({
            "last_slot": self.last_slot,
            "alert_id": self.alert_id,
            "reported": self.reported,
        })
        .to_string()
    }

    /// Reads state written by [`SelfTest::to_json`]. The error never repeats the input.
    pub fn from_json(json: &str) -> Result<Self, &'static str> {
        let v: serde_json::Value = serde_json::from_str(json).map_err(|_| "not JSON")?;
        let last_slot = match v.get("last_slot") {
            None | Some(serde_json::Value::Null) => None,
            Some(n) => Some(n.as_u64().ok_or("last_slot is not a number")?),
        };
        let alert_id = match v.get("alert_id") {
            None | Some(serde_json::Value::Null) => None,
            Some(s) => Some(s.as_str().ok_or("alert_id is not text")?.to_string()),
        };
        let reported = v
            .get("reported")
            .and_then(serde_json::Value::as_bool)
            .ok_or("reported is not true or false")?;
        Ok(SelfTest {
            last_slot,
            alert_id,
            reported,
        })
    }

    /// The unix time the next [`SelfTest::tick`] has something to do: now if a test is due or the
    /// pending one has already ended, else the sooner of the next daily slot and the pending test's
    /// deadline. The caller sleeps until then (or until a signal); waking early is harmless.
    pub fn next_wake(
        &self,
        store: &dyn AlertStore,
        now: u64,
        at_secs: u64,
        deadline_secs: u64,
    ) -> u64 {
        let slot = latest_slot(now, at_secs);
        if self.last_slot.is_none_or(|last| slot > last) {
            return now;
        }
        let mut wake = slot + DAY;
        if !self.reported
            && let Some(id) = self.alert_id.as_deref()
        {
            match store.get(id) {
                None => return now,
                Some(r) if r.state == AlertState::Acked => return now,
                Some(r) => wake = wake.min(r.sent_at + deadline_secs),
            }
        }
        wake.max(now)
    }
}

/// Where the sender's failure signal goes. The channel is not chosen here and is not the phone:
/// it may be the broken part. Implemented once the channel is decided (tied to A5).
pub trait FailureNotifier {
    /// A test ended in a failure verdict (`alert_id` is the test's id).
    fn verdict_failed(&self, verdict: Verdict, alert_id: Option<&str>);
    /// Today's test could not be raised at all.
    fn raise_failed(&self, error: &RaiseError);
}

/// Sends every failure in `tick` to `notifier` and returns how many it sent. A passed test and a
/// successful raise send nothing.
pub fn notify_failures(
    tick: &Tick,
    alert_id: Option<&str>,
    notifier: &dyn FailureNotifier,
) -> usize {
    let mut sent = 0;
    if let Some(v) = tick.verdict
        && v.is_failure()
    {
        notifier.verdict_failed(v, alert_id);
        sent += 1;
    }
    if let Some(Err(e)) = &tick.raised {
        notifier.raise_failed(e);
        sent += 1;
    }
    sent
}

/// Durable home for [`SelfTest`] state (a PersistAnt-backed implementation comes with the service).
pub trait SelfTestStore {
    fn load(&self) -> Option<String>;
    fn save(&self, json: &str);
}

/// In-memory [`SelfTestStore`], for tests and until PersistAnt is wired.
#[derive(Debug, Default)]
pub struct MemorySelfTestStore(std::sync::Mutex<Option<String>>);

impl SelfTestStore for MemorySelfTestStore {
    fn load(&self) -> Option<String> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
    fn save(&self, json: &str) {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(json.to_string());
    }
}

/// One scheduler step: load the stored state (missing or unreadable state starts afresh, which
/// raises a test now rather than none), run [`SelfTest::tick`], save the state, send failures to
/// `notifier`. Returns the tick and when to call again: `retry_secs` after a failed raise, else
/// [`SelfTest::next_wake`].
#[allow(clippy::too_many_arguments)]
pub async fn run_step(
    state: &dyn SelfTestStore,
    notifier: &dyn FailureNotifier,
    devices: &dyn DeviceRegistry,
    transport: &impl PushTransport,
    store: &dyn AlertStore,
    now: u64,
    at_secs: u64,
    deadline_secs: u64,
    retry_secs: u64,
) -> (Tick, u64) {
    let mut st = state
        .load()
        .and_then(|j| SelfTest::from_json(&j).ok())
        .unwrap_or_default();
    // The verdict is about the test that was pending before this tick, not one raised during it.
    let previous_id = st.alert_id.clone();
    let tick = st
        .tick(devices, transport, store, now, at_secs, deadline_secs)
        .await;
    state.save(&st.to_json());
    notify_failures(&tick, previous_id.as_deref(), notifier);
    let wake = if matches!(tick.raised, Some(Err(_))) {
        now + retry_secs.max(1)
    } else {
        st.next_wake(store, now, at_secs, deadline_secs)
    };
    (tick, wake)
}

/// Longest single sleep in [`run_loop`]. A wake time far away is reached in hops, so a clock
/// that jumps or a sleep that ends early costs one extra step, never a missed test.
pub const MAX_SLEEP_SECS: u64 = 3600;

/// When the daily test runs and how long it waits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Schedule {
    /// Seconds after 00:00 UTC of the daily slot.
    pub at_secs: u64,
    /// Seconds a test may wait for its acknowledge before it counts as missed.
    pub deadline_secs: u64,
    /// Seconds to wait before retrying a failed raise.
    pub retry_secs: u64,
}

/// Runs [`run_step`] until `stop` says so, sleeping between steps for `sleep(secs)`: the time to
/// the step's wake, at least one second (never a busy loop) and at most [`MAX_SLEEP_SECS`].
/// Returns the number of steps run. Runtime-agnostic: the caller supplies the clock (unix
/// seconds), the sleep (a tokio sleep, in the service) and the stop flag, which is checked
/// before every step and again before every sleep.
#[allow(clippy::too_many_arguments)]
pub async fn run_loop<Fut: Future<Output = ()>>(
    state: &dyn SelfTestStore,
    notifier: &dyn FailureNotifier,
    devices: &dyn DeviceRegistry,
    transport: &impl PushTransport,
    store: &dyn AlertStore,
    schedule: &Schedule,
    clock: &dyn Fn() -> u64,
    sleep: impl Fn(u64) -> Fut,
    stop: &dyn Fn() -> bool,
) -> u64 {
    let mut steps = 0;
    while !stop() {
        let now = clock();
        let (_, wake) = run_step(
            state,
            notifier,
            devices,
            transport,
            store,
            now,
            schedule.at_secs,
            schedule.deadline_secs,
            schedule.retry_secs,
        )
        .await;
        steps += 1;
        if stop() {
            break;
        }
        sleep(wake.saturating_sub(now).clamp(1, MAX_SLEEP_SECS)).await;
    }
    steps
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ack::{AckOutcome, AckToken, MemoryStore, acknowledge};
    use crate::fcm::{AlertMessage, DeviceToken, PushError, PushReceipt};
    use crate::raise::Device;
    use std::future::Future;
    use std::sync::Mutex;
    use std::task::{Context, Poll, Waker};

    fn block_on<F: Future>(f: F) -> F::Output {
        let mut f = std::pin::pin!(f);
        let mut cx = Context::from_waker(Waker::noop());
        loop {
            if let Poll::Ready(v) = f.as_mut().poll(&mut cx) {
                return v;
            }
        }
    }

    #[derive(Default)]
    struct Fake {
        fail: Mutex<bool>,
        sent: Mutex<Vec<AlertMessage>>,
    }

    impl PushTransport for Fake {
        fn send(
            &self,
            _: &DeviceToken,
            alert: &AlertMessage,
        ) -> impl Future<Output = Result<PushReceipt, PushError>> + Send {
            let r = if *self.fail.lock().unwrap() {
                Err(PushError::Transient("down".into()))
            } else {
                self.sent.lock().unwrap().push(alert.clone());
                Ok(PushReceipt {
                    message_name: "m".into(),
                })
            };
            async move { r }
        }
    }

    struct One;

    impl DeviceRegistry for One {
        fn device(&self) -> Option<Device> {
            Some(Device {
                push_token: DeviceToken::new("t"),
                ack_token: AckToken::new("ack"),
            })
        }
    }

    const AT: u64 = 7 * 3600; // 07:00 UTC
    const DEADLINE: u64 = 3600;
    const DAY0: u64 = 20_000 * DAY;

    fn tick(st: &mut SelfTest, t: &Fake, s: &MemoryStore, now: u64) -> Tick {
        block_on(st.tick(&One, t, s, now, AT, DEADLINE))
    }

    #[test]
    fn slot_is_the_latest_daily_time_not_after_now() {
        assert_eq!(latest_slot(DAY0 + AT, AT), DAY0 + AT);
        assert_eq!(latest_slot(DAY0 + AT + 5, AT), DAY0 + AT);
        assert_eq!(latest_slot(DAY0 + AT - 1, AT), DAY0 - DAY + AT);
        assert_eq!(latest_slot(DAY0 + DAY - 1, AT), DAY0 + AT);
    }

    #[test]
    fn raises_once_per_day_with_the_fixed_text() {
        let (t, s, mut st) = (Fake::default(), MemoryStore::default(), SelfTest::default());
        let first = tick(&mut st, &t, &s, DAY0 + AT + 1);
        let id = first.raised.unwrap().unwrap();
        assert_eq!(first.verdict, None);
        assert!(tick(&mut st, &t, &s, DAY0 + AT + 60).raised.is_none());
        let sent = t.sent.lock().unwrap();
        assert_eq!(sent.len(), 1);
        assert_eq!(
            (sent[0].alert_id.as_str(), sent[0].title.as_str()),
            (id.as_str(), TITLE)
        );
        assert_eq!(sent[0].body, BODY);
    }

    #[test]
    fn an_acknowledged_test_passes_once_then_the_next_day_raises_again() {
        let (t, s, mut st) = (Fake::default(), MemoryStore::default(), SelfTest::default());
        let id = tick(&mut st, &t, &s, DAY0 + AT).raised.unwrap().unwrap();
        assert_eq!(
            acknowledge(&s, &id, Some("Bearer ack"), DAY0 + AT + 30),
            AckOutcome::Acked
        );
        let mid = tick(&mut st, &t, &s, DAY0 + AT + 100);
        assert_eq!((mid.verdict, mid.raised), (Some(Verdict::Passed), None));
        assert_eq!(
            tick(&mut st, &t, &s, DAY0 + AT + 200).verdict,
            None,
            "reported once"
        );
        let next = tick(&mut st, &t, &s, DAY0 + DAY + AT);
        assert!(next.raised.unwrap().is_ok());
        assert_eq!(t.sent.lock().unwrap().len(), 2);
    }

    #[test]
    fn an_unacknowledged_test_fails_only_after_the_deadline_and_once() {
        let (t, s, mut st) = (Fake::default(), MemoryStore::default(), SelfTest::default());
        tick(&mut st, &t, &s, DAY0 + AT);
        assert_eq!(
            tick(&mut st, &t, &s, DAY0 + AT + DEADLINE - 1).verdict,
            None
        );
        let late = tick(&mut st, &t, &s, DAY0 + AT + DEADLINE);
        assert_eq!(late.verdict, Some(Verdict::NotAcked));
        assert!(late.verdict.unwrap().is_failure());
        assert_eq!(
            tick(&mut st, &t, &s, DAY0 + AT + DEADLINE + 60).verdict,
            None
        );
    }

    #[test]
    fn a_test_the_sender_forgot_is_lost() {
        let (t, s, mut st) = (Fake::default(), MemoryStore::default(), SelfTest::default());
        let id = tick(&mut st, &t, &s, DAY0 + AT).raised.unwrap().unwrap();
        s.remove(&id);
        assert_eq!(
            tick(&mut st, &t, &s, DAY0 + AT + 10).verdict,
            Some(Verdict::Lost)
        );
    }

    #[test]
    fn a_failed_raise_is_reported_and_retried_next_tick() {
        let (t, s, mut st) = (Fake::default(), MemoryStore::default(), SelfTest::default());
        *t.fail.lock().unwrap() = true;
        let bad = tick(&mut st, &t, &s, DAY0 + AT);
        assert_eq!(bad.raised, Some(Err(RaiseError::Transient)));
        assert_eq!(st.last_slot, None);
        *t.fail.lock().unwrap() = false;
        assert!(
            tick(&mut st, &t, &s, DAY0 + AT + 60)
                .raised
                .unwrap()
                .is_ok()
        );
    }

    #[derive(Default)]
    struct Note(Mutex<Vec<String>>);

    impl FailureNotifier for Note {
        fn verdict_failed(&self, v: Verdict, id: Option<&str>) {
            self.0.lock().unwrap().push(format!("{v:?} {id:?}"));
        }
        fn raise_failed(&self, e: &RaiseError) {
            self.0.lock().unwrap().push(format!("raise {e:?}"));
        }
    }

    fn step(
        ss: &MemorySelfTestStore,
        n: &Note,
        t: &Fake,
        s: &MemoryStore,
        now: u64,
    ) -> (Tick, u64) {
        block_on(run_step(ss, n, &One, t, s, now, AT, DEADLINE, 120))
    }

    #[test]
    fn state_round_trips_and_bad_state_is_refused_without_echo() {
        let st = SelfTest {
            last_slot: Some(DAY0 + AT),
            alert_id: Some("abc".into()),
            reported: true,
        };
        assert_eq!(SelfTest::from_json(&st.to_json()), Ok(st));
        let fresh = SelfTest::default();
        assert_eq!(SelfTest::from_json(&fresh.to_json()), Ok(fresh));
        for bad in [
            "",
            "{",
            "[]",
            r#"{"reported":"yes"}"#,
            r#"{"reported":true,"last_slot":"secret-x"}"#,
            r#"{"reported":true,"alert_id":5}"#,
        ] {
            let e = SelfTest::from_json(bad).unwrap_err();
            assert!(!e.contains("secret-x"), "{bad:?}");
        }
    }

    #[test]
    fn next_wake_follows_the_day_and_the_pending_deadline() {
        let (t, s, mut st) = (Fake::default(), MemoryStore::default(), SelfTest::default());
        assert_eq!(
            st.next_wake(&s, DAY0 + 5, AT, DEADLINE),
            DAY0 + 5,
            "due now"
        );
        tick(&mut st, &t, &s, DAY0 + AT);
        // Pending test: wake at its deadline, which is before tomorrow's slot.
        assert_eq!(
            st.next_wake(&s, DAY0 + AT + 10, AT, DEADLINE),
            DAY0 + AT + DEADLINE
        );
        tick(&mut st, &t, &s, DAY0 + AT + DEADLINE);
        // Reported: wake at tomorrow's slot.
        assert_eq!(
            st.next_wake(&s, DAY0 + AT + DEADLINE + 1, AT, DEADLINE),
            DAY0 + DAY + AT
        );
        // A test the sender forgot, or that was acked, is ready to report now.
        let (mut st2, s2) = (SelfTest::default(), MemoryStore::default());
        let id = tick(&mut st2, &t, &s2, DAY0 + AT).raised.unwrap().unwrap();
        acknowledge(&s2, &id, Some("Bearer ack"), DAY0 + AT + 5);
        assert_eq!(
            st2.next_wake(&s2, DAY0 + AT + 6, AT, DEADLINE),
            DAY0 + AT + 6
        );
        s2.remove(&id);
        assert_eq!(
            st2.next_wake(&s2, DAY0 + AT + 7, AT, DEADLINE),
            DAY0 + AT + 7
        );
    }

    #[test]
    fn run_step_persists_between_restarts_and_does_not_raise_twice() {
        let (ss, n, t, s) = (
            MemorySelfTestStore::default(),
            Note::default(),
            Fake::default(),
            MemoryStore::default(),
        );
        let (first, wake) = step(&ss, &n, &t, &s, DAY0 + AT);
        assert!(first.raised.unwrap().is_ok());
        assert_eq!(wake, DAY0 + AT + DEADLINE);
        // "Restart": nothing in memory but the store; the same day raises nothing more.
        let (again, _) = step(&ss, &n, &t, &s, DAY0 + AT + 60);
        assert!(again.raised.is_none());
        assert_eq!(t.sent.lock().unwrap().len(), 1);
        assert!(n.0.lock().unwrap().is_empty());
    }

    #[test]
    fn run_step_notifies_a_missed_ack_once_with_the_ended_tests_id() {
        let (ss, n, t, s) = (
            MemorySelfTestStore::default(),
            Note::default(),
            Fake::default(),
            MemoryStore::default(),
        );
        step(&ss, &n, &t, &s, DAY0 + AT);
        let id = SelfTest::from_json(&ss.load().unwrap())
            .unwrap()
            .alert_id
            .unwrap();
        step(&ss, &n, &t, &s, DAY0 + AT + DEADLINE);
        step(&ss, &n, &t, &s, DAY0 + AT + DEADLINE + 60);
        assert_eq!(*n.0.lock().unwrap(), vec![format!("NotAcked Some({id:?})")]);
        // Next day: the new test is raised in the same tick that nothing is reported for.
        step(&ss, &n, &t, &s, DAY0 + DAY + AT);
        assert_eq!(n.0.lock().unwrap().len(), 1);
    }

    #[test]
    fn a_verdict_and_a_new_raise_in_one_tick_name_the_ended_test() {
        let (ss, n, t, s) = (
            MemorySelfTestStore::default(),
            Note::default(),
            Fake::default(),
            MemoryStore::default(),
        );
        step(&ss, &n, &t, &s, DAY0 + AT);
        let old = SelfTest::from_json(&ss.load().unwrap())
            .unwrap()
            .alert_id
            .unwrap();
        // The service was down for a day: the old test is overdue and tomorrow's is due.
        let (tk, _) = step(&ss, &n, &t, &s, DAY0 + DAY + AT);
        let new = tk.raised.unwrap().unwrap();
        assert_ne!(new, old);
        assert_eq!(tk.verdict, Some(Verdict::NotAcked));
        assert_eq!(
            *n.0.lock().unwrap(),
            vec![format!("NotAcked Some({old:?})")]
        );
    }

    #[test]
    fn a_passed_test_notifies_nobody() {
        let (ss, n, t, s) = (
            MemorySelfTestStore::default(),
            Note::default(),
            Fake::default(),
            MemoryStore::default(),
        );
        let (first, _) = step(&ss, &n, &t, &s, DAY0 + AT);
        let id = first.raised.unwrap().unwrap();
        acknowledge(&s, &id, Some("Bearer ack"), DAY0 + AT + 5);
        let (second, _) = step(&ss, &n, &t, &s, DAY0 + AT + 10);
        assert_eq!(second.verdict, Some(Verdict::Passed));
        assert!(n.0.lock().unwrap().is_empty());
    }

    #[test]
    fn a_failed_raise_is_notified_and_retried_after_the_retry_delay() {
        let (ss, n, t, s) = (
            MemorySelfTestStore::default(),
            Note::default(),
            Fake::default(),
            MemoryStore::default(),
        );
        *t.fail.lock().unwrap() = true;
        let (bad, wake) = step(&ss, &n, &t, &s, DAY0 + AT);
        assert_eq!(bad.raised, Some(Err(RaiseError::Transient)));
        assert_eq!(wake, DAY0 + AT + 120);
        assert_eq!(*n.0.lock().unwrap(), vec!["raise Transient".to_string()]);
        *t.fail.lock().unwrap() = false;
        let (ok, _) = step(&ss, &n, &t, &s, wake);
        assert!(ok.raised.unwrap().is_ok());
    }

    #[test]
    fn unreadable_stored_state_starts_afresh_and_raises_a_test() {
        let (ss, n, t, s) = (
            MemorySelfTestStore::default(),
            Note::default(),
            Fake::default(),
            MemoryStore::default(),
        );
        ss.save("garbage");
        let (tk, _) = step(&ss, &n, &t, &s, DAY0 + AT);
        assert!(tk.raised.unwrap().is_ok());
        assert!(SelfTest::from_json(&ss.load().unwrap()).is_ok());
    }

    #[test]
    fn a_missed_day_does_not_raise_two_tests() {
        let (t, s, mut st) = (Fake::default(), MemoryStore::default(), SelfTest::default());
        tick(&mut st, &t, &s, DAY0 + AT);
        tick(&mut st, &t, &s, DAY0 + 3 * DAY + AT + 5);
        assert_eq!(t.sent.lock().unwrap().len(), 2);
    }

    const SCHED: Schedule = Schedule {
        at_secs: AT,
        deadline_secs: DEADLINE,
        retry_secs: 120,
    };

    /// Runs the loop on a simulated clock that `sleep` advances; stops after `n` sleeps.
    fn run_for(
        n: usize,
        start: u64,
        ss: &MemorySelfTestStore,
        note: &Note,
        t: &Fake,
        s: &MemoryStore,
    ) -> (u64, Vec<u64>) {
        let clock = std::cell::Cell::new(start);
        let sleeps = std::cell::RefCell::new(Vec::new());
        let steps = block_on(run_loop(
            ss,
            note,
            &One,
            t,
            s,
            &SCHED,
            &|| clock.get(),
            |secs| {
                sleeps.borrow_mut().push(secs);
                clock.set(clock.get() + secs);
                async {}
            },
            &|| sleeps.borrow().len() >= n,
        ));
        (steps, sleeps.into_inner())
    }

    fn fresh() -> (MemorySelfTestStore, Note, Fake, MemoryStore) {
        (
            MemorySelfTestStore::default(),
            Note::default(),
            Fake::default(),
            MemoryStore::default(),
        )
    }

    #[test]
    fn loop_raises_once_notifies_a_miss_once_and_sleeps_in_capped_hops() {
        let (ss, n, t, s) = fresh();
        let (steps, sleeps) = run_for(3, DAY0 + AT, &ss, &n, &t, &s);
        assert_eq!(steps, 3);
        assert_eq!(sleeps, vec![3600, 3600, 3600]);
        assert_eq!(t.sent.lock().unwrap().len(), 1);
        assert_eq!(n.0.lock().unwrap().len(), 1, "one missed ack, one notice");
    }

    #[test]
    fn loop_retries_a_failed_raise_after_the_retry_delay() {
        let (ss, n, t, s) = fresh();
        *t.fail.lock().unwrap() = true;
        let (_, sleeps) = run_for(2, DAY0 + AT, &ss, &n, &t, &s);
        assert_eq!(sleeps, vec![120, 120]);
        assert_eq!(
            n.0.lock().unwrap().len(),
            2,
            "each failed raise is reported"
        );
    }

    #[test]
    fn loop_stopped_up_front_runs_nothing() {
        let (ss, n, t, s) = fresh();
        let (steps, sleeps) = run_for(0, DAY0 + AT, &ss, &n, &t, &s);
        assert_eq!((steps, sleeps.len()), (0, 0));
        assert!(t.sent.lock().unwrap().is_empty());
        assert!(ss.load().is_none());
    }
}
