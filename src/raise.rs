//! A9: the one call other projects make to raise an alert.
//!
//! `POST /alert` with `Authorization: Bearer <api key>` and a JSON body `{"title": "...",
//! "body": "..."}`. Answers `202` with `{"alert_id": "..."}` once the push service has accepted the
//! message. The caller keeps the id; the phone's acknowledgement arrives at `POST /ack/{id}` (A4).
//! Framework-free like [`crate::ack`]: the HTTP server, clock and device registry plug in.

use serde_json::{Value, json};

use crate::AlertState;
use crate::ack::{AckToken, AlertRecord, AlertStore, bearer_token};
use crate::fcm::{AlertMessage, DeviceToken, PushError, PushTransport};

/// Longest accepted title, in characters. A lock screen shows about this much.
pub const MAX_TITLE_CHARS: usize = 100;
/// Longest accepted body, in characters. FCM data messages are capped at 4 KB in total.
pub const MAX_BODY_CHARS: usize = 1000;

/// The phone an alert goes to. One device per recipient (PATENT-SWEEP constraint 10).
#[derive(Debug, Clone)]
pub struct Device {
    pub push_token: DeviceToken,
    /// What this phone presents when it acknowledges.
    pub ack_token: AckToken,
}

/// Where the registered device comes from. A PersistAnt-backed registry replaces fixed config later.
pub trait DeviceRegistry {
    fn device(&self) -> Option<Device>;
}

/// Why a raise failed, with the HTTP status to answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RaiseError {
    /// Missing or wrong API key.
    Unauthorized,
    /// Body is not the documented JSON, or a field is empty or too long.
    BadRequest(&'static str),
    /// No phone has registered yet.
    NoDevice,
    /// The push service is unreachable or busy; the caller may retry.
    Transient,
    /// The phone's push token is no longer valid; it must register again.
    DeviceGone,
    /// The push service refused the message; retrying will not help.
    Rejected,
}

impl RaiseError {
    pub fn status(&self) -> u16 {
        match self {
            RaiseError::Unauthorized => 401,
            RaiseError::BadRequest(_) => 400,
            RaiseError::NoDevice => 409,
            RaiseError::Transient => 503,
            RaiseError::DeviceGone => 410,
            RaiseError::Rejected => 502,
        }
    }
}

/// A validated request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RaiseRequest {
    pub title: String,
    pub body: String,
}

/// Parses and validates the request body. Unknown fields are ignored; both fields must be strings.
pub fn parse_request(json: &str) -> Result<RaiseRequest, RaiseError> {
    let v: Value =
        serde_json::from_str(json).map_err(|_| RaiseError::BadRequest("body is not JSON"))?;
    let field = |name: &'static str| -> Result<String, RaiseError> {
        v.get(name)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or(RaiseError::BadRequest(match name {
                "title" => "title must be a string",
                _ => "body must be a string",
            }))
    };
    let (title, body) = (field("title")?, field("body")?);
    if title.trim().is_empty() {
        return Err(RaiseError::BadRequest("title is empty"));
    }
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(RaiseError::BadRequest("title is too long"));
    }
    if body.chars().count() > MAX_BODY_CHARS {
        return Err(RaiseError::BadRequest("body is too long"));
    }
    Ok(RaiseRequest { title, body })
}

/// A new alert id: 128 random bits as hex. Unguessable, so it can sit in a URL.
pub fn new_alert_id() -> String {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).expect("the operating system's random source is unavailable");
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Handles `POST /alert`. Returns the alert id on success. The record is stored before the push is
/// sent, so an acknowledgement that races the response still finds it.
pub async fn raise_alert(
    api_key: &AckToken,
    authorization: Option<&str>,
    body: &str,
    devices: &dyn DeviceRegistry,
    transport: &impl PushTransport,
    store: &dyn AlertStore,
    now: u64,
) -> Result<String, RaiseError> {
    let authorised = authorization
        .and_then(bearer_token)
        .is_some_and(|t| api_key.matches(t));
    if !authorised {
        return Err(RaiseError::Unauthorized);
    }
    raise_authorised(body, devices, transport, store, now).await
}

/// [`raise_alert`] for a caller already trusted (the daily self-test, inside the service).
pub async fn raise_authorised(
    body: &str,
    devices: &dyn DeviceRegistry,
    transport: &impl PushTransport,
    store: &dyn AlertStore,
    now: u64,
) -> Result<String, RaiseError> {
    let req = parse_request(body)?;
    let device = devices.device().ok_or(RaiseError::NoDevice)?;
    let alert_id = new_alert_id();
    store.insert(AlertRecord {
        alert_id: alert_id.clone(),
        state: AlertState::Sent,
        sent_at: now,
        acked_at: None,
        ack_token: device.ack_token,
    });
    let message = AlertMessage {
        alert_id: alert_id.clone(),
        title: req.title,
        body: req.body,
    };
    match transport.send(&device.push_token, &message).await {
        Ok(_) => Ok(alert_id),
        Err(e) => {
            store.remove(&alert_id);
            Err(match e {
                PushError::Transient(_) => RaiseError::Transient,
                PushError::TokenGone => RaiseError::DeviceGone,
                PushError::Permanent(_) => RaiseError::Rejected,
            })
        }
    }
}

/// The status and JSON body to answer for a [`raise_alert`] result.
pub fn response(result: &Result<String, RaiseError>) -> (u16, Value) {
    match result {
        Ok(id) => (202, json!({ "alert_id": id })),
        Err(RaiseError::BadRequest(why)) => (400, json!({ "error": why })),
        Err(e) => (e.status(), json!({ "error": format!("{e:?}") })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ack::MemoryStore;
    use crate::fcm::PushReceipt;
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
        sent: Mutex<Vec<AlertMessage>>,
    }

    impl PushTransport for Fake {
        fn send(
            &self,
            _: &DeviceToken,
            alert: &AlertMessage,
        ) -> impl Future<Output = Result<PushReceipt, PushError>> + Send {
            let r = match &self.fail {
                Some(e) => Err(e.clone()),
                None => {
                    self.sent.lock().unwrap().push(alert.clone());
                    Ok(PushReceipt {
                        message_name: "projects/p/messages/1".into(),
                    })
                }
            };
            async move { r }
        }
    }

    struct One(Option<Device>);

    impl DeviceRegistry for One {
        fn device(&self) -> Option<Device> {
            self.0.clone()
        }
    }

    fn device() -> One {
        One(Some(Device {
            push_token: DeviceToken::new("fcm-tok"),
            ack_token: AckToken::new("ack-tok"),
        }))
    }

    const KEY: Option<&str> = Some("Bearer key");
    const OK_BODY: &str = r#"{"title":"DB down","body":"prod-1 unreachable"}"#;

    fn run(
        auth: Option<&str>,
        body: &str,
        devices: &One,
        t: &Fake,
        s: &MemoryStore,
    ) -> Result<String, RaiseError> {
        block_on(raise_alert(
            &AckToken::new("key"),
            auth,
            body,
            devices,
            t,
            s,
            50,
        ))
    }

    #[test]
    fn raising_stores_a_sent_record_and_pushes_exactly_the_text() {
        let (t, s) = (Fake::default(), MemoryStore::default());
        let id = run(KEY, OK_BODY, &device(), &t, &s).unwrap();
        assert_eq!(id.len(), 32);
        let r = s.get(&id).unwrap();
        assert_eq!((r.state, r.sent_at), (AlertState::Sent, 50));
        assert!(r.ack_token.matches("ack-tok"));
        let sent = t.sent.lock().unwrap();
        assert_eq!(sent.len(), 1);
        assert_eq!(
            sent[0],
            AlertMessage {
                alert_id: id,
                title: "DB down".into(),
                body: "prod-1 unreachable".into()
            }
        );
    }

    #[test]
    fn the_phones_token_then_acknowledges_the_raised_alert() {
        let (t, s) = (Fake::default(), MemoryStore::default());
        let id = run(KEY, OK_BODY, &device(), &t, &s).unwrap();
        let out = crate::ack::acknowledge(&s, &id, Some("Bearer ack-tok"), 90);
        assert_eq!(out, crate::ack::AckOutcome::Acked);
    }

    #[test]
    fn wrong_or_missing_key_is_401_and_sends_nothing() {
        for auth in [None, Some("Bearer nope"), Some("key")] {
            let (t, s) = (Fake::default(), MemoryStore::default());
            let e = run(auth, OK_BODY, &device(), &t, &s).unwrap_err();
            assert_eq!(e, RaiseError::Unauthorized);
            assert_eq!(e.status(), 401);
            assert!(t.sent.lock().unwrap().is_empty());
        }
    }

    #[test]
    fn bad_bodies_are_400() {
        let long_title = format!(r#"{{"title":"{}","body":""}}"#, "x".repeat(101));
        let long_body = format!(r#"{{"title":"t","body":"{}"}}"#, "x".repeat(1001));
        for body in [
            "",
            "not json",
            "[]",
            r#"{"title":"t"}"#,
            r#"{"body":"b"}"#,
            r#"{"title":1,"body":"b"}"#,
            r#"{"title":"  ","body":"b"}"#,
            long_title.as_str(),
            long_body.as_str(),
        ] {
            let (t, s) = (Fake::default(), MemoryStore::default());
            let e = run(KEY, body, &device(), &t, &s).unwrap_err();
            assert!(matches!(e, RaiseError::BadRequest(_)), "{body:?}");
            assert!(t.sent.lock().unwrap().is_empty());
        }
    }

    #[test]
    fn limits_are_inclusive_and_counted_in_characters() {
        let ok = format!(
            r#"{{"title":"{}","body":"{}"}}"#,
            "é".repeat(MAX_TITLE_CHARS),
            "é".repeat(MAX_BODY_CHARS)
        );
        let (t, s) = (Fake::default(), MemoryStore::default());
        assert!(run(KEY, &ok, &device(), &t, &s).is_ok());
    }

    #[test]
    fn no_registered_device_is_409() {
        let (t, s) = (Fake::default(), MemoryStore::default());
        let e = run(KEY, OK_BODY, &One(None), &t, &s).unwrap_err();
        assert_eq!((e.clone(), e.status()), (RaiseError::NoDevice, 409));
    }

    #[test]
    fn push_failures_map_to_statuses_and_leave_no_record() {
        for (fail, want, status) in [
            (PushError::Transient("x".into()), RaiseError::Transient, 503),
            (PushError::TokenGone, RaiseError::DeviceGone, 410),
            (PushError::Permanent("x".into()), RaiseError::Rejected, 502),
        ] {
            let (t, s) = (
                Fake {
                    fail: Some(fail),
                    ..Fake::default()
                },
                MemoryStore::default(),
            );
            let e = run(KEY, OK_BODY, &device(), &t, &s).unwrap_err();
            assert_eq!((e.clone(), e.status()), (want, status));
            assert!(s.is_empty(), "a failed push must not leave a Sent record");
        }
    }

    #[test]
    fn ids_are_unique() {
        let ids: std::collections::HashSet<_> = (0..1000).map(|_| new_alert_id()).collect();
        assert_eq!(ids.len(), 1000);
    }

    #[test]
    fn response_shapes() {
        assert_eq!(
            response(&Ok("abc".into())),
            (202, json!({"alert_id": "abc"}))
        );
        assert_eq!(
            response(&Err(RaiseError::BadRequest("title is empty"))),
            (400, json!({"error": "title is empty"}))
        );
        assert_eq!(response(&Err(RaiseError::Unauthorized)).0, 401);
    }
}
