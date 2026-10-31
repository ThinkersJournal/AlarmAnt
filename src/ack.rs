//! A4: the acknowledge call back and per-alert state on the sender.
//!
//! The phone calls `POST /ack/{alert_id}` with `Authorization: Bearer <ack token>` (`AckWorker`).
//! This module is the framework-free handler behind that route: it checks the token, moves the
//! alert to [`AlertState::Acked`] exactly once and says what to answer. The HTTP server (APIAnt,
//! when it ships), the clock and the durable store (PersistAnt) plug in behind [`AlertStore`] and
//! the `now` argument. The caller applies `AlertEvent::Ack` to the A5 state machine and records
//! `alert.acked` when the outcome is [`AckOutcome::Acked`].

use std::collections::HashMap;
use std::fmt;
use std::sync::Mutex;

use crate::AlertState;

/// The secret a phone presents to acknowledge an alert sent to it. `Debug` and `Display` never
/// show it.
#[derive(Clone, PartialEq, Eq)]
pub struct AckToken(String);

impl AckToken {
    pub fn new(token: impl Into<String>) -> Self {
        AckToken(token.into())
    }

    /// Compares in time that does not depend on where the first difference is.
    pub fn matches(&self, presented: &str) -> bool {
        let (a, b) = (self.0.as_bytes(), presented.as_bytes());
        let mut diff = a.len() ^ b.len();
        for i in 0..a.len().max(b.len()) {
            diff |= usize::from(a.get(i).copied().unwrap_or(0) ^ b.get(i).copied().unwrap_or(0));
        }
        diff == 0
    }
}

impl fmt::Debug for AckToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AckToken(<redacted>)")
    }
}

impl fmt::Display for AckToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

/// What the sender remembers about one alert.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlertRecord {
    pub alert_id: String,
    pub state: AlertState,
    /// Unix seconds when the push was accepted.
    pub sent_at: u64,
    /// Unix seconds when the first acknowledgement arrived.
    pub acked_at: Option<u64>,
    /// The token of the device the alert was sent to.
    pub ack_token: AckToken,
}

impl AlertRecord {
    /// A record for an alert just sent to a device.
    pub fn sent(alert_id: impl Into<String>, sent_at: u64, ack_token: AckToken) -> Self {
        AlertRecord {
            alert_id: alert_id.into(),
            state: AlertState::Sent,
            sent_at,
            acked_at: None,
            ack_token,
        }
    }
}

/// Durable per-alert state. A PersistAnt-backed implementation comes with the service.
pub trait AlertStore {
    fn insert(&self, record: AlertRecord);
    fn get(&self, alert_id: &str) -> Option<AlertRecord>;
    /// Moves the alert to `Acked` at `at` if it is not already. Must be atomic, so two racing
    /// calls report `Some(true)` once. `None`: no such alert. `Some(false)`: already acked.
    fn mark_acked(&self, alert_id: &str, at: u64) -> Option<bool>;
}

/// In-memory [`AlertStore`], for tests and a single-process service until PersistAnt is wired.
#[derive(Debug, Default)]
pub struct MemoryStore {
    records: Mutex<HashMap<String, AlertRecord>>,
}

impl MemoryStore {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, AlertRecord>> {
        self.records.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl AlertStore for MemoryStore {
    fn insert(&self, record: AlertRecord) {
        self.lock().insert(record.alert_id.clone(), record);
    }

    fn get(&self, alert_id: &str) -> Option<AlertRecord> {
        self.lock().get(alert_id).cloned()
    }

    fn mark_acked(&self, alert_id: &str, at: u64) -> Option<bool> {
        let mut records = self.lock();
        let r = records.get_mut(alert_id)?;
        if r.state == AlertState::Acked {
            return Some(false);
        }
        r.state = AlertState::Acked;
        r.acked_at = Some(at);
        Some(true)
    }
}

/// What an acknowledge request did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AckOutcome {
    /// First acknowledgement: apply `Ack` to the state machine and audit `alert.acked`.
    Acked,
    /// A repeat (the phone retried): nothing to do, still a success for the phone.
    AlreadyAcked,
    /// Missing, malformed or wrong token.
    Unauthorized,
    /// No such alert (the sender lost it, or the id is wrong). Retrying will not help.
    NotFound,
}

impl AckOutcome {
    /// The HTTP status to answer. 2xx tells the phone to stop retrying; 401 and 404 are final.
    pub fn status(self) -> u16 {
        match self {
            AckOutcome::Acked | AckOutcome::AlreadyAcked => 200,
            AckOutcome::Unauthorized => 401,
            AckOutcome::NotFound => 404,
        }
    }
}

/// The token in an `Authorization: Bearer <token>` header value.
pub fn bearer_token(authorization: &str) -> Option<&str> {
    let (scheme, rest) = authorization.split_once(' ')?;
    let token = rest.trim();
    (scheme.eq_ignore_ascii_case("bearer") && !token.is_empty()).then_some(token)
}

/// Handles `POST /ack/{alert_id}`. `authorization` is the raw header value, if any; `now` is the
/// current unix time in seconds.
pub fn acknowledge(
    store: &dyn AlertStore,
    alert_id: &str,
    authorization: Option<&str>,
    now: u64,
) -> AckOutcome {
    let Some(record) = store.get(alert_id) else {
        return AckOutcome::NotFound;
    };
    let ok = authorization
        .and_then(bearer_token)
        .is_some_and(|t| record.ack_token.matches(t));
    if !ok {
        return AckOutcome::Unauthorized;
    }
    match store.mark_acked(alert_id, now) {
        Some(true) => AckOutcome::Acked,
        Some(false) => AckOutcome::AlreadyAcked,
        None => AckOutcome::NotFound,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn store() -> MemoryStore {
        let s = MemoryStore::default();
        s.insert(AlertRecord::sent("a-1", 100, AckToken::new("s3cret")));
        s
    }

    const GOOD: Option<&str> = Some("Bearer s3cret");

    #[test]
    fn first_ack_moves_sent_to_acked_and_records_the_time() {
        let s = store();
        assert_eq!(acknowledge(&s, "a-1", GOOD, 160), AckOutcome::Acked);
        let r = s.get("a-1").unwrap();
        assert_eq!(r.state, AlertState::Acked);
        assert_eq!(r.acked_at, Some(160));
        assert_eq!(r.sent_at, 100);
    }

    #[test]
    fn a_repeat_is_a_success_and_keeps_the_first_time() {
        let s = store();
        acknowledge(&s, "a-1", GOOD, 160);
        assert_eq!(acknowledge(&s, "a-1", GOOD, 999), AckOutcome::AlreadyAcked);
        assert_eq!(AckOutcome::AlreadyAcked.status(), 200);
        assert_eq!(s.get("a-1").unwrap().acked_at, Some(160));
    }

    #[test]
    fn wrong_missing_or_malformed_token_is_401_and_changes_nothing() {
        let s = store();
        for auth in [
            None,
            Some(""),
            Some("Bearer"),
            Some("Bearer "),
            Some("Bearer wrong"),
            Some("Bearer s3cret2"),
            Some("Bearer s3cre"),
            Some("Basic s3cret"),
            Some("s3cret"),
        ] {
            let out = acknowledge(&s, "a-1", auth, 5);
            assert_eq!(out, AckOutcome::Unauthorized, "{auth:?}");
            assert_eq!(out.status(), 401);
        }
        assert_eq!(s.get("a-1").unwrap().state, AlertState::Sent);
    }

    #[test]
    fn unknown_alert_is_404() {
        let s = store();
        let out = acknowledge(&s, "nope", GOOD, 5);
        assert_eq!(out, AckOutcome::NotFound);
        assert_eq!(out.status(), 404);
    }

    #[test]
    fn one_device_cannot_ack_another_devices_alert() {
        let s = store();
        s.insert(AlertRecord::sent("a-2", 100, AckToken::new("other")));
        assert_eq!(acknowledge(&s, "a-2", GOOD, 5), AckOutcome::Unauthorized);
        assert_eq!(s.get("a-2").unwrap().state, AlertState::Sent);
    }

    #[test]
    fn ack_ends_an_escalated_or_unresolved_alert() {
        for state in [AlertState::Escalated, AlertState::Unresolved] {
            let s = MemoryStore::default();
            let mut r = AlertRecord::sent("a", 1, AckToken::new("t"));
            r.state = state;
            s.insert(r);
            assert_eq!(acknowledge(&s, "a", Some("Bearer t"), 9), AckOutcome::Acked);
            assert_eq!(s.get("a").unwrap().state, AlertState::Acked);
        }
    }

    #[test]
    fn racing_acks_report_acked_once() {
        let s = Arc::new(store());
        let handles: Vec<_> = (0..16)
            .map(|i| {
                let s = Arc::clone(&s);
                std::thread::spawn(move || acknowledge(&*s, "a-1", GOOD, 200 + i))
            })
            .collect();
        let outs: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert_eq!(outs.iter().filter(|o| **o == AckOutcome::Acked).count(), 1);
        assert_eq!(
            outs.iter()
                .filter(|o| **o == AckOutcome::AlreadyAcked)
                .count(),
            15
        );
    }

    #[test]
    fn bearer_parsing() {
        assert_eq!(bearer_token("Bearer abc"), Some("abc"));
        assert_eq!(bearer_token("bearer abc"), Some("abc"));
        assert_eq!(bearer_token("BEARER  abc "), Some("abc"));
        assert_eq!(bearer_token("Bearer"), None);
        assert_eq!(bearer_token("Token abc"), None);
    }

    #[test]
    fn token_never_prints() {
        let t = AckToken::new("s3cret");
        let r = AlertRecord::sent("a", 1, t.clone());
        assert!(!format!("{t:?} {t} {r:?}").contains("s3cret"));
    }

    #[test]
    fn matches_is_exact() {
        let t = AckToken::new("abc");
        assert!(t.matches("abc"));
        assert!(!t.matches("abd"));
        assert!(!t.matches("ab"));
        assert!(!t.matches("abcd"));
        assert!(!t.matches(""));
    }
}
