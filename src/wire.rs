//! A5 wiring: escalation hooked into raising (A9) and acknowledging (A4).
//!
//! [`raise_escalating`] is [`crate::raise::raise_alert`] plus starting the escalation timer;
//! [`acknowledge_escalating`] is [`crate::ack::acknowledge`] plus cancelling it. No provider is
//! wired here: the second-app/PC-alarm [`AlertChannel`] is passed in, and the phone's only input is still
//! the acknowledgement.
//!
//! Ordering matters. An acknowledgement can race the raise response, so the raise path checks the
//! store for an acknowledgement and registers under the escalator lock, and the ack path cancels
//! under that same lock only after the store says acknowledged. Whichever runs first, an acked
//! alert never keeps a live timer.

use std::fmt;
use std::sync::Mutex;

use crate::ack::{AckOutcome, AckToken, AlertStore, acknowledge};
use crate::escalate::{DEFAULT_MINUTES, Escalator};
use crate::fcm::PushTransport;
use crate::raise::{DeviceRegistry, RaiseError, raise_alert};
use crate::{AlertChannel, AlertState, Outcome};

/// Longest wait the single setting accepts, in minutes.
pub const MAX_MINUTES: u64 = 120;

/// Why escalation settings were refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigError {
    /// `minutes` outside `1..=MAX_MINUTES`.
    Minutes,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Minutes => write!(f, "minutes must be 1 to {MAX_MINUTES}"),
        }
    }
}

impl std::error::Error for ConfigError {}

/// The escalation settings: one number `N`. Nothing else is configurable
/// (DESIGN-A5: no schedules, no battery gate, no per-step timers).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EscalationConfig {
    minutes: u64,
}

impl EscalationConfig {
    /// `minutes` of 1 to 120.
    pub fn new(minutes: u64) -> Result<Self, ConfigError> {
        if !(1..=MAX_MINUTES).contains(&minutes) {
            return Err(ConfigError::Minutes);
        }
        Ok(EscalationConfig { minutes })
    }

    /// Same, with the default `N` (5 minutes).
    pub fn with_default_minutes() -> Result<Self, ConfigError> {
        Self::new(DEFAULT_MINUTES)
    }

    pub fn minutes(&self) -> u64 {
        self.minutes
    }
}

fn lock(e: &Mutex<Escalator>) -> std::sync::MutexGuard<'_, Escalator> {
    e.lock().unwrap_or_else(|p| p.into_inner())
}

/// `POST /alert` with escalation: raises the alert, then starts its timer unless the phone has
/// already acknowledged.
#[allow(clippy::too_many_arguments)]
pub async fn raise_escalating(
    escalator: &Mutex<Escalator>,
    api_key: &AckToken,
    authorization: Option<&str>,
    body: &str,
    devices: &dyn DeviceRegistry,
    transport: &impl PushTransport,
    store: &dyn AlertStore,
    now: u64,
) -> Result<String, RaiseError> {
    let id = raise_alert(api_key, authorization, body, devices, transport, store, now).await?;
    let mut e = lock(escalator);
    let acked = store.get(&id).is_some_and(|r| r.state == AlertState::Acked);
    if !acked {
        e.register(&id, now);
    }
    Ok(id)
}

/// `POST /ack/{id}` with escalation: on a first or repeated acknowledgement, cancels the timer.
/// The second value holds the state-machine outcome (audit `alert.acked`) when a live escalation
/// was ended by this call.
pub fn acknowledge_escalating(
    escalator: &Mutex<Escalator>,
    channel: &dyn AlertChannel,
    store: &dyn AlertStore,
    alert_id: &str,
    authorization: Option<&str>,
    now: u64,
) -> (AckOutcome, Option<Outcome>) {
    let outcome = acknowledge(store, alert_id, authorization, now);
    let ended = match outcome {
        AckOutcome::Acked | AckOutcome::AlreadyAcked => lock(escalator).ack(alert_id, channel),
        AckOutcome::Unauthorized | AckOutcome::NotFound => None,
    };
    (outcome, ended)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AuditEvent;
    use crate::ack::MemoryStore;
    use crate::escalate::MemoryEscalationStore;
    use crate::fcm::{AlertMessage, DeviceToken, PushError, PushReceipt};
    use crate::raise::Device;
    use std::future::Future;
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

    struct Ok1;
    #[allow(clippy::manual_async_fn)]
    impl PushTransport for Ok1 {
        fn send(
            &self,
            _: &DeviceToken,
            _: &AlertMessage,
        ) -> impl Future<Output = Result<PushReceipt, PushError>> + Send {
            async {
                Ok(PushReceipt {
                    message_name: "m".into(),
                })
            }
        }
    }

    struct Down;
    #[allow(clippy::manual_async_fn)]
    impl PushTransport for Down {
        fn send(
            &self,
            _: &DeviceToken,
            _: &AlertMessage,
        ) -> impl Future<Output = Result<PushReceipt, PushError>> + Send {
            async { Err(PushError::Transient("down".into())) }
        }
    }

    struct One;
    impl DeviceRegistry for One {
        fn device(&self) -> Option<Device> {
            Some(Device {
                push_token: DeviceToken::new("fcm"),
                ack_token: AckToken::new("ack-tok"),
            })
        }
    }

    #[derive(Default)]
    struct Chan(Mutex<Vec<&'static str>>);
    impl AlertChannel for Chan {
        fn second_app(&self, _: &str) -> Result<(), crate::ChannelError> {
            self.0.lock().unwrap().push("second_app");
            Ok(())
        }
        fn pc_alarm(&self, _: &str) -> Result<(), crate::ChannelError> {
            self.0.lock().unwrap().push("pc_alarm");
            Ok(())
        }
    }

    const BODY: &str = r#"{"title":"DB down","body":"prod-1"}"#;
    const KEY: Option<&str> = Some("Bearer key");
    const ACK: Option<&str> = Some("Bearer ack-tok");

    fn raise(
        e: &Mutex<Escalator>,
        t: &impl PushTransport,
        s: &MemoryStore,
    ) -> Result<String, RaiseError> {
        block_on(raise_escalating(
            e,
            &AckToken::new("key"),
            KEY,
            BODY,
            &One,
            t,
            s,
            100,
        ))
    }

    fn escalator() -> Mutex<Escalator> {
        Mutex::new(Escalator::with_store(
            5,
            Box::new(std::sync::Arc::new(MemoryEscalationStore::default())),
        ))
    }

    #[test]
    fn raise_starts_the_timer_and_unacked_alert_escalates() {
        let (e, s, ch) = (escalator(), MemoryStore::default(), Chan::default());
        let id = raise(&e, &Ok1, &s).unwrap();
        assert_eq!(lock(&e).state(&id), Some(AlertState::Sent));
        assert_eq!(lock(&e).next_due(), Some(400));
        lock(&e).tick(400, &ch);
        assert_eq!(*ch.0.lock().unwrap(), vec!["second_app"]);
    }

    #[test]
    fn failed_raise_starts_no_timer() {
        let (e, s) = (escalator(), MemoryStore::default());
        assert_eq!(raise(&e, &Down, &s), Err(RaiseError::Transient));
        assert_eq!(lock(&e).next_due(), None);
        let (e, s) = (escalator(), MemoryStore::default());
        let r = block_on(raise_escalating(
            &e,
            &AckToken::new("key"),
            Some("Bearer wrong"),
            BODY,
            &One,
            &Ok1,
            &s,
            100,
        ));
        assert_eq!(r, Err(RaiseError::Unauthorized));
        assert_eq!(lock(&e).next_due(), None);
    }

    #[test]
    fn ack_cancels_the_timer_and_nothing_is_sent() {
        let (e, s, ch) = (escalator(), MemoryStore::default(), Chan::default());
        let id = raise(&e, &Ok1, &s).unwrap();
        let (out, ended) = acknowledge_escalating(&e, &ch, &s, &id, ACK, 150);
        assert_eq!(out, AckOutcome::Acked);
        assert_eq!(ended.unwrap().audit, vec![AuditEvent::Acked]);
        assert_eq!(lock(&e).next_due(), None);
        assert!(lock(&e).tick(10_000, &ch).is_empty());
        assert!(ch.0.lock().unwrap().is_empty());
        // A retry from the phone is still a 200 and changes nothing.
        let (out, ended) = acknowledge_escalating(&e, &ch, &s, &id, ACK, 160);
        assert_eq!((out, ended), (AckOutcome::AlreadyAcked, None));
    }

    #[test]
    fn bad_token_or_unknown_alert_leaves_the_timer_running() {
        let (e, s, ch) = (escalator(), MemoryStore::default(), Chan::default());
        let id = raise(&e, &Ok1, &s).unwrap();
        let (out, ended) = acknowledge_escalating(&e, &ch, &s, &id, Some("Bearer no"), 150);
        assert_eq!((out, ended), (AckOutcome::Unauthorized, None));
        let (out, _) = acknowledge_escalating(&e, &ch, &s, "nope", ACK, 150);
        assert_eq!(out, AckOutcome::NotFound);
        assert_eq!(lock(&e).state(&id), Some(AlertState::Sent));
    }

    #[test]
    fn ack_that_beat_the_registration_starts_no_timer() {
        // The store already says Acked when the raise path reaches its check.
        let (e, s) = (escalator(), MemoryStore::default());
        struct Racing<'a>(&'a MemoryStore);
        impl PushTransport for Racing<'_> {
            fn send(
                &self,
                _: &DeviceToken,
                m: &AlertMessage,
            ) -> impl Future<Output = Result<PushReceipt, PushError>> + Send {
                self.0.mark_acked(&m.alert_id, 101);
                async {
                    Ok(PushReceipt {
                        message_name: "m".into(),
                    })
                }
            }
        }
        let id = raise(&e, &Racing(&s), &s).unwrap();
        assert_eq!(s.get(&id).unwrap().state, AlertState::Acked);
        assert_eq!(lock(&e).state(&id), None);
        assert_eq!(lock(&e).next_due(), None);
    }

    #[test]
    fn config_accepts_one_to_120_minutes_and_defaults_to_5() {
        assert_eq!(EscalationConfig::new(0), Err(ConfigError::Minutes));
        assert_eq!(EscalationConfig::new(121), Err(ConfigError::Minutes));
        assert_eq!(EscalationConfig::new(120).unwrap().minutes(), 120);
        assert_eq!(
            EscalationConfig::with_default_minutes().unwrap().minutes(),
            5
        );
    }
}
