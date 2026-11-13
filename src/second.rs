//! Three-channel step 1b: the second contact's app push (ruling 389, tier 2).
//!
//! When the first timeout fires, [`push_second`] sends the same alert text to the second contact's
//! phone and records that phone's ack token on the alert, so either contact's acknowledgement ends
//! it ([`crate::ack::acknowledge`] accepts both). The timer is the escalator's, not this
//! function's: nothing here waits, retries, or reads a reply, and the phone still sends nothing
//! but an Acknowledge (constraint 13). Nothing here logs or carries a token.

use crate::ack::AlertStore;
use crate::fcm::{AlertMessage, PushError, PushTransport};
use crate::raise::DeviceRegistry;

/// Why the second push did not go out. For the audit trail; the escalator's chain advances
/// regardless.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecondError {
    /// No such alert.
    NoAlert,
    /// Already acknowledged: nothing to push.
    AlreadyAcked,
    /// No second contact is registered.
    NoDevice,
    /// The alert's text was not kept (cannot happen for an alert raised by this crate).
    NoText,
    /// The push service is unreachable or busy.
    Transient,
    /// The second phone's push token is no longer valid; it must register again.
    DeviceGone,
    /// The push service refused the message.
    Rejected,
}

/// Pushes alert `alert_id` to the second contact and stores their ack token on it.
pub async fn push_second(
    devices: &dyn DeviceRegistry,
    transport: &impl PushTransport,
    store: &dyn AlertStore,
    alert_id: &str,
) -> Result<(), SecondError> {
    let record = store.get(alert_id).ok_or(SecondError::NoAlert)?;
    if record.state == crate::AlertState::Acked {
        return Err(SecondError::AlreadyAcked);
    }
    let text = record.text.ok_or(SecondError::NoText)?;
    let device = devices.second_device().ok_or(SecondError::NoDevice)?;
    if !store.set_second_ack_token(alert_id, device.ack_token.clone()) {
        return Err(SecondError::AlreadyAcked);
    }
    // A revocation can land between the read and the store, after it voided what it could see.
    if !devices
        .second_device()
        .is_some_and(|d| d.ack_token.same_digest(&device.ack_token))
    {
        store.void_ack_tokens(&device.ack_token);
    }
    let message = AlertMessage {
        alert_id: alert_id.to_owned(),
        title: text.title,
        body: text.body,
    };
    transport
        .send(&device.push_token, &message)
        .await
        .map(|_| ())
        .map_err(|e| match e {
            PushError::Transient(_) => SecondError::Transient,
            PushError::TokenGone => SecondError::DeviceGone,
            PushError::Permanent(_) => SecondError::Rejected,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AlertState;
    use crate::ack::{AckOutcome, AckToken, MemoryStore, acknowledge};
    use crate::fcm::{DeviceToken, PushReceipt};
    use crate::raise::{Device, raise_authorised};
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
        fail: Option<PushError>,
        sent: Mutex<Vec<(String, AlertMessage)>>,
    }

    impl PushTransport for Fake {
        fn send(
            &self,
            to: &DeviceToken,
            alert: &AlertMessage,
        ) -> impl Future<Output = Result<PushReceipt, PushError>> + Send {
            let r = match &self.fail {
                Some(e) => Err(e.clone()),
                None => {
                    self.sent
                        .lock()
                        .unwrap()
                        .push((format!("{to:?}"), alert.clone()));
                    Ok(PushReceipt {
                        message_name: "m".into(),
                    })
                }
            };
            async move { r }
        }
    }

    fn dev(push: &str, ack: &str) -> Device {
        Device {
            push_token: DeviceToken::new(push),
            ack_token: AckToken::new(ack),
        }
    }

    struct Two {
        first: Option<Device>,
        second: Mutex<Option<Device>>,
        /// Revoke the second device right after the first read of it.
        vanish: bool,
    }

    impl Two {
        fn new() -> Self {
            Two {
                first: Some(dev("fcm-1", "ack-1")),
                second: Mutex::new(Some(dev("fcm-2", "ack-2"))),
                vanish: false,
            }
        }
    }

    impl DeviceRegistry for Two {
        fn device(&self) -> Option<Device> {
            self.first.clone()
        }
        fn second_device(&self) -> Option<Device> {
            let mut g = self.second.lock().unwrap();
            if self.vanish { g.take() } else { g.clone() }
        }
    }

    const BODY: &str = r#"{"title":"DB down","body":"prod-1"}"#;

    fn raised(reg: &Two, s: &MemoryStore) -> String {
        block_on(raise_authorised(BODY, reg, &Fake::default(), s, 10)).unwrap()
    }

    fn ack(s: &MemoryStore, id: &str, tok: &str) -> AckOutcome {
        acknowledge(s, id, Some(&format!("Bearer {tok}")), 20)
    }

    #[test]
    fn the_second_contact_gets_the_same_text_and_either_token_then_acks() {
        let (reg, s, t) = (Two::new(), MemoryStore::default(), Fake::default());
        let id = raised(&reg, &s);
        assert_eq!(block_on(push_second(&reg, &t, &s, &id)), Ok(()));
        let sent = t.sent.lock().unwrap();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].1.title, "DB down");
        assert_eq!(sent[0].1.body, "prod-1");
        assert_eq!(sent[0].1.alert_id, id);
        assert_eq!(ack(&s, &id, "ack-2"), AckOutcome::Acked);
    }

    #[test]
    fn the_first_contacts_token_still_acks_after_the_second_push() {
        let (reg, s, t) = (Two::new(), MemoryStore::default(), Fake::default());
        let id = raised(&reg, &s);
        block_on(push_second(&reg, &t, &s, &id)).unwrap();
        assert_eq!(ack(&s, &id, "ack-1"), AckOutcome::Acked);
        assert_eq!(s.get(&id).unwrap().state, AlertState::Acked);
    }

    #[test]
    fn the_second_token_does_not_work_before_the_second_push() {
        let (reg, s) = (Two::new(), MemoryStore::default());
        let id = raised(&reg, &s);
        assert_eq!(ack(&s, &id, "ack-2"), AckOutcome::Unauthorized);
    }

    #[test]
    fn a_wrong_token_is_still_refused_after_the_second_push() {
        let (reg, s, t) = (Two::new(), MemoryStore::default(), Fake::default());
        let id = raised(&reg, &s);
        block_on(push_second(&reg, &t, &s, &id)).unwrap();
        assert_eq!(ack(&s, &id, "ack-3"), AckOutcome::Unauthorized);
    }

    #[test]
    fn an_acknowledged_alert_is_not_pushed_again() {
        let (reg, s, t) = (Two::new(), MemoryStore::default(), Fake::default());
        let id = raised(&reg, &s);
        assert_eq!(ack(&s, &id, "ack-1"), AckOutcome::Acked);
        assert_eq!(
            block_on(push_second(&reg, &t, &s, &id)),
            Err(SecondError::AlreadyAcked)
        );
        assert!(t.sent.lock().unwrap().is_empty());
    }

    #[test]
    fn no_second_contact_means_no_push_and_no_second_token() {
        let (reg, s, t) = (Two::new(), MemoryStore::default(), Fake::default());
        *reg.second.lock().unwrap() = None;
        let id = raised(&reg, &s);
        assert_eq!(
            block_on(push_second(&reg, &t, &s, &id)),
            Err(SecondError::NoDevice)
        );
        assert!(t.sent.lock().unwrap().is_empty());
        assert!(s.get(&id).unwrap().second_ack_token.is_none());
    }

    #[test]
    fn an_unknown_alert_is_refused() {
        let (reg, s, t) = (Two::new(), MemoryStore::default(), Fake::default());
        assert_eq!(
            block_on(push_second(&reg, &t, &s, "nope")),
            Err(SecondError::NoAlert)
        );
    }

    #[test]
    fn push_failures_are_reported_by_kind() {
        for (e, want) in [
            (PushError::Transient("x".into()), SecondError::Transient),
            (PushError::TokenGone, SecondError::DeviceGone),
            (PushError::Permanent("x".into()), SecondError::Rejected),
        ] {
            let (reg, s) = (Two::new(), MemoryStore::default());
            let t = Fake {
                fail: Some(e),
                ..Fake::default()
            };
            let id = raised(&reg, &s);
            assert_eq!(block_on(push_second(&reg, &t, &s, &id)), Err(want));
        }
    }

    #[test]
    fn revoking_the_second_device_voids_its_token_on_in_flight_alerts() {
        let (reg, s, t) = (Two::new(), MemoryStore::default(), Fake::default());
        let id = raised(&reg, &s);
        block_on(push_second(&reg, &t, &s, &id)).unwrap();
        assert_eq!(s.void_ack_tokens(&AckToken::new("ack-2")), 1);
        assert_eq!(ack(&s, &id, "ack-2"), AckOutcome::Unauthorized);
        assert_eq!(
            ack(&s, &id, "ack-1"),
            AckOutcome::Acked,
            "the first is untouched"
        );
    }

    #[test]
    fn a_push_racing_a_revocation_of_the_second_device_leaves_no_working_token() {
        let (mut reg, s, t) = (Two::new(), MemoryStore::default(), Fake::default());
        let id = raised(&reg, &s);
        reg.vanish = true;
        block_on(push_second(&reg, &t, &s, &id)).unwrap();
        assert_eq!(ack(&s, &id, "ack-2"), AckOutcome::Unauthorized);
    }
}
