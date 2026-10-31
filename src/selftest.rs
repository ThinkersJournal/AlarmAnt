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

    #[test]
    fn a_missed_day_does_not_raise_two_tests() {
        let (t, s, mut st) = (Fake::default(), MemoryStore::default(), SelfTest::default());
        tick(&mut st, &t, &s, DAY0 + AT);
        tick(&mut st, &t, &s, DAY0 + 3 * DAY + AT + 5);
        assert_eq!(t.sent.lock().unwrap().len(), 2);
    }
}
