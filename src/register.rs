//! Registration slice (c): `POST /register` (DESIGN-REGISTRATION.md).
//!
//! The phone presents the one-time enrolment code ([`crate::enroll`]) as `Authorization: Bearer
//! <code>` with `{"fcm_token": "..."}`. A right, unexpired, unused code makes the phone the device:
//! a fresh 256-bit ack token is generated, only its digest is kept, and the token is answered once
//! (`201 {"ack_token": "..."}`). Framework-free like [`crate::ack`] and [`crate::raise`].
//!
//! Failed attempts answer one fixed 401 whatever was wrong. The limiter counts **failures only**
//! (5 in 60 s, then 429 instead of 401): the code is always checked first, so a valid code passes
//! however many bad ones came before it, and a flood of bad codes cannot lock out a real
//! enrolment. (Guessing is not what the limiter prevents: a 128-bit code cannot be guessed; the
//! limiter bounds noise and gives the operator a signal.)
//! Nothing here logs, and no error echoes input.

use std::collections::VecDeque;
use std::sync::Mutex;

use serde_json::{Value, json};

use crate::ack::{AckToken, bearer_token};
use crate::enroll::{Contact, EnrollmentStore};
use crate::fcm::DeviceToken;
use crate::raise::{Device, DeviceRegistry};

/// Failed attempts allowed in [`FAIL_WINDOW_SECS`] before the answer becomes 429.
pub const MAX_FAILURES: usize = 5;
/// The sliding window for failed attempts, in seconds.
pub const FAIL_WINDOW_SECS: u64 = 60;
/// Longest accepted FCM token, in characters (real ones are about 160).
pub const MAX_FCM_TOKEN_CHARS: usize = 4096;

/// A device registry that can also be written. A PersistAnt-backed one replaces memory later.
pub trait DeviceStore: DeviceRegistry {
    /// Makes `d` the device for `contact`, replacing any earlier one in that slot.
    fn set_for(&self, contact: Contact, d: Device);
    /// [`set_for`](Self::set_for) the first contact.
    fn set(&self, d: Device) {
        self.set_for(Contact::First, d);
    }
    /// Replaces the push token if `presented_ack` is a registered device's ack token (either
    /// contact's); the ack token itself is kept. False (and nothing changed) when no device
    /// holds that token. One step, so a concurrent re-registration cannot be overwritten.
    fn replace_push_token(&self, presented_ack: &str, token: DeviceToken) -> bool;
    /// Removes `contact`'s device record and returns it. Revocation only: nothing restores it.
    fn take_for(&self, contact: Contact) -> Option<Device>;
    /// [`take_for`](Self::take_for) the first contact.
    fn take(&self) -> Option<Device> {
        self.take_for(Contact::First)
    }
}

/// In-memory [`DeviceStore`].
#[derive(Debug, Default)]
pub struct MemoryDevices {
    devices: Mutex<[Option<Device>; 2]>,
}

impl MemoryDevices {
    fn lock(&self) -> std::sync::MutexGuard<'_, [Option<Device>; 2]> {
        self.devices.lock().unwrap_or_else(|p| p.into_inner())
    }
}

impl MemoryDevices {
    pub(crate) fn snapshot(&self) -> [Option<Device>; 2] {
        self.lock().clone()
    }
}

impl DeviceRegistry for MemoryDevices {
    fn device(&self) -> Option<Device> {
        self.lock()[Contact::First.index()].clone()
    }

    fn second_device(&self) -> Option<Device> {
        self.lock()[Contact::Second.index()].clone()
    }
}

impl DeviceStore for MemoryDevices {
    fn set_for(&self, contact: Contact, d: Device) {
        self.lock()[contact.index()] = Some(d);
    }

    fn take_for(&self, contact: Contact) -> Option<Device> {
        self.lock()[contact.index()].take()
    }

    fn replace_push_token(&self, presented_ack: &str, token: DeviceToken) -> bool {
        let mut g = self.lock();
        // Both slots are always checked, so timing does not say which contact matched.
        let mut hit = None;
        for (i, slot) in g.iter().enumerate() {
            if slot
                .as_ref()
                .is_some_and(|d| d.ack_token.matches(presented_ack))
            {
                hit = Some(i);
            }
        }
        match hit.and_then(|i| g[i].as_mut()) {
            Some(d) => {
                d.push_token = token;
                true
            }
            None => false,
        }
    }
}

/// Counts failed registration attempts in a sliding window.
#[derive(Debug, Default)]
pub struct FailureLimiter {
    failures: Mutex<VecDeque<u64>>,
}

impl FailureLimiter {
    /// Records a failure at `now`; true when this one is over the limit.
    pub fn record_failure(&self, now: u64) -> bool {
        let mut f = self.failures.lock().unwrap_or_else(|p| p.into_inner());
        while f
            .front()
            .is_some_and(|t| now.saturating_sub(*t) >= FAIL_WINDOW_SECS)
        {
            f.pop_front();
        }
        f.push_back(now);
        f.len() > MAX_FAILURES
    }
}

/// Why a registration failed, with the HTTP status to answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterError {
    /// The body is not the documented JSON or the token is unusable.
    BadRequest(&'static str),
    /// Missing, wrong, used or expired code.
    Unauthorized,
    /// Too many failed attempts lately.
    TooManyFailures,
}

impl RegisterError {
    pub fn status(self) -> u16 {
        match self {
            RegisterError::BadRequest(_) => 400,
            RegisterError::Unauthorized => 401,
            RegisterError::TooManyFailures => 429,
        }
    }
}

/// A new ack token: 256 random bits as 64 lowercase hex digits.
pub fn new_ack_token() -> String {
    let mut b = [0u8; 32];
    getrandom::fill(&mut b).expect("the operating system's random source is unavailable");
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn parse_fcm_token(body: &str) -> Result<String, RegisterError> {
    let v: Value =
        serde_json::from_str(body).map_err(|_| RegisterError::BadRequest("body is not JSON"))?;
    let t = v
        .get("fcm_token")
        .and_then(Value::as_str)
        .ok_or(RegisterError::BadRequest("fcm_token must be a string"))?;
    if t.is_empty() || t.chars().count() > MAX_FCM_TOKEN_CHARS || t.chars().any(char::is_control) {
        return Err(RegisterError::BadRequest("fcm_token is not usable"));
    }
    Ok(t.to_owned())
}

/// Handles `POST /register`; returns the new ack token, to be answered once.
///
/// The body is checked before the code is spent, so a malformed request cannot waste a code; a 400
/// says nothing about the code.
pub fn register(
    enrollments: &dyn EnrollmentStore,
    devices: &dyn DeviceStore,
    limiter: &FailureLimiter,
    authorization: Option<&str>,
    body: &str,
    now: u64,
) -> Result<String, RegisterError> {
    let fcm = parse_fcm_token(body)?;
    // The code is always checked, so a valid one passes whatever came before it; only a failure
    // is counted, and the count only changes which refusal is answered.
    let contact = authorization
        .and_then(bearer_token)
        .and_then(|code| enrollments.consume(code, now).ok());
    let Some(contact) = contact else {
        return Err(if limiter.record_failure(now) {
            RegisterError::TooManyFailures
        } else {
            RegisterError::Unauthorized
        });
    };
    let token = new_ack_token();
    devices.set_for(
        contact,
        Device {
            push_token: DeviceToken::new(fcm),
            ack_token: AckToken::new(&token),
        },
    );
    Ok(token)
}

/// Handles `PUT /device/token` (FCM token rotation): `Authorization: Bearer <ack token>` and
/// `{"fcm_token"}`. The ack token is the credential, so no operator step is needed; it is checked
/// first, so a stranger learns nothing about the body, and the token is replaced in one step with
/// the check. Only the push token changes: nothing here reports delivery or escalates anything.
pub fn update_token(
    devices: &dyn DeviceStore,
    authorization: Option<&str>,
    body: &str,
) -> Result<(), RegisterError> {
    let presented = authorization
        .and_then(bearer_token)
        .ok_or(RegisterError::Unauthorized)?;
    // Parsed here but applied only if the credential matches; an unauthenticated caller gets the
    // 401 either way, so the body is never judged for a stranger.
    let parsed = parse_fcm_token(body);
    match parsed {
        Ok(fcm) => {
            if devices.replace_push_token(presented, DeviceToken::new(fcm)) {
                Ok(())
            } else {
                Err(RegisterError::Unauthorized)
            }
        }
        Err(bad) => {
            if [devices.device(), devices.second_device()]
                .iter()
                .flatten()
                .any(|d| d.ack_token.matches(presented))
            {
                Err(bad)
            } else {
                Err(RegisterError::Unauthorized)
            }
        }
    }
}

/// The status and JSON body to answer. Fixed text: nothing the client sent is repeated.
pub fn response(result: &Result<String, RegisterError>) -> (u16, Value) {
    match result {
        Ok(t) => (201, json!({ "ack_token": t })),
        Err(RegisterError::BadRequest(why)) => (400, json!({ "error": why })),
        Err(RegisterError::Unauthorized) => (401, json!({ "error": "unauthorized" })),
        Err(RegisterError::TooManyFailures) => {
            (429, json!({ "error": "too many failed attempts" }))
        }
    }
}

/// The status and JSON body for `PUT /device/token`: 200, or the same fixed failures as `/register`.
pub fn token_response(result: &Result<(), RegisterError>) -> (u16, Value) {
    match result {
        Ok(()) => (200, json!({ "updated": true })),
        Err(e) => response(&Err(*e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enroll::{ENROLL_TTL_SECS, MemoryEnrollments, new_enrollment};

    const BODY: &str = r#"{"fcm_token":"tok-abc"}"#;

    struct Rig {
        enr: MemoryEnrollments,
        dev: MemoryDevices,
        lim: FailureLimiter,
        code: String,
    }

    fn rig() -> Rig {
        let enr = MemoryEnrollments::default();
        let (code, e) = new_enrollment(1000);
        enr.set(e);
        Rig {
            enr,
            dev: MemoryDevices::default(),
            lim: FailureLimiter::default(),
            code,
        }
    }

    fn go(r: &Rig, auth: Option<&str>, body: &str, now: u64) -> Result<String, RegisterError> {
        register(&r.enr, &r.dev, &r.lim, auth, body, now)
    }

    fn bearer(c: &str) -> String {
        format!("Bearer {c}")
    }

    fn registered() -> (Rig, String) {
        let r = rig();
        let token = go(&r, Some(&bearer(&r.code)), BODY, 1001).unwrap();
        (r, token)
    }

    fn put(r: &Rig, auth: Option<&str>, body: &str) -> Result<(), RegisterError> {
        update_token(&r.dev, auth, body)
    }

    const NEW_BODY: &str = r#"{"fcm_token":"tok-rotated"}"#;

    #[test]
    fn the_ack_token_replaces_the_push_token_and_keeps_acking() {
        let (r, token) = registered();
        put(&r, Some(&bearer(&token)), NEW_BODY).unwrap();
        let d = r.dev.device().unwrap();
        assert_eq!(d.push_token.expose(), "tok-rotated");
        assert!(d.ack_token.matches(&token), "the ack token is unchanged");
        assert_eq!(token_response(&Ok(())).0, 200);
    }

    #[test]
    fn a_missing_wrong_or_borrowed_credential_is_401_and_changes_nothing() {
        let (r, token) = registered();
        let digest = r.dev.device().unwrap().ack_token.digest_hex();
        let code_like = bearer(&r.code);
        let bearer_ok = bearer(&token);
        for auth in [
            None,
            Some("Bearer nope"),
            Some(token.as_str()),
            Some(code_like.as_str()),
            Some(&bearer(&digest)[..]),
            Some("Basic x"),
        ] {
            let e = put(&r, auth, NEW_BODY).unwrap_err();
            assert_eq!(e, RegisterError::Unauthorized, "{auth:?}");
            assert_eq!(r.dev.device().unwrap().push_token.expose(), "tok-abc");
        }
        assert!(put(&r, Some(&bearer_ok), NEW_BODY).is_ok());
    }

    #[test]
    fn with_no_device_registered_it_is_401() {
        let r = rig();
        let e = put(&r, Some("Bearer anything"), NEW_BODY).unwrap_err();
        assert_eq!(e, RegisterError::Unauthorized);
        assert!(r.dev.device().is_none());
    }

    #[test]
    fn credentials_are_checked_before_the_body_and_a_bad_body_changes_nothing() {
        let (r, token) = registered();
        let e = put(&r, Some("Bearer nope"), "not json").unwrap_err();
        assert_eq!(
            e,
            RegisterError::Unauthorized,
            "no body detail for strangers"
        );
        for body in [
            "not json",
            "{}",
            r#"{"fcm_token":""}"#,
            r#"{"fcm_token":5}"#,
        ] {
            let e = put(&r, Some(&bearer(&token)), body).unwrap_err();
            assert!(matches!(e, RegisterError::BadRequest(_)), "{body}");
            assert_eq!(r.dev.device().unwrap().push_token.expose(), "tok-abc");
        }
    }

    #[test]
    fn a_valid_code_registers_the_phone_and_the_token_acks_for_it() {
        let r = rig();
        let token = go(&r, Some(&bearer(&r.code)), BODY, 1001).unwrap();
        assert_eq!(token.len(), 64);
        let d = r.dev.device().unwrap();
        assert!(d.ack_token.matches(&token));
        assert!(!d.ack_token.matches(&d.ack_token.digest_hex()));
        assert_eq!(d.push_token.expose(), "tok-abc");
        assert_eq!(response(&Ok(token.clone())).0, 201);
    }

    #[test]
    fn missing_wrong_reused_and_expired_codes_are_401_and_register_nothing() {
        let r = rig();
        for auth in [
            None,
            Some("Bearer nope"),
            Some("Basic x"),
            Some(r.code.as_str()),
        ] {
            let e = go(&r, auth, BODY, 1001).unwrap_err();
            assert_eq!(e, RegisterError::Unauthorized, "{auth:?}");
            assert!(r.dev.device().is_none());
        }
        // reused
        go(&r, Some(&bearer(&r.code)), BODY, 1002).unwrap();
        r.dev.set(Device {
            push_token: DeviceToken::new("old"),
            ack_token: AckToken::new("old-ack"),
        });
        let e = go(&r, Some(&bearer(&r.code)), BODY, 1003).unwrap_err();
        assert_eq!(e, RegisterError::Unauthorized);
        assert_eq!(r.dev.device().unwrap().push_token.expose(), "old");
        // expired
        let r = rig();
        let at = 1000 + ENROLL_TTL_SECS;
        let e = go(&r, Some(&bearer(&r.code)), BODY, at).unwrap_err();
        assert_eq!(e, RegisterError::Unauthorized);
        assert!(r.dev.device().is_none());
    }

    #[test]
    fn the_sixth_failure_in_a_minute_is_429_and_old_failures_age_out() {
        let r = rig();
        for i in 0..MAX_FAILURES {
            let e = go(&r, Some("Bearer bad"), BODY, 1001 + i as u64).unwrap_err();
            assert_eq!(e, RegisterError::Unauthorized, "attempt {i}");
        }
        let e = go(&r, Some("Bearer bad"), BODY, 1010).unwrap_err();
        assert_eq!(e, RegisterError::TooManyFailures);
        assert_eq!(e.status(), 429);
        // a minute after the first failure the window has moved on
        let e = go(&r, Some("Bearer bad"), BODY, 1001 + FAIL_WINDOW_SECS + 5).unwrap_err();
        assert_eq!(e, RegisterError::Unauthorized);
    }

    #[test]
    fn a_valid_code_passes_after_a_flood_of_bad_ones() {
        let r = rig();
        for i in 0..200 {
            let _ = go(&r, Some("Bearer bad"), BODY, 1001 + (i % 3));
        }
        assert!(go(&r, Some(&bearer(&r.code)), BODY, 1004).is_ok());
    }

    #[test]
    fn a_bad_body_is_400_and_does_not_spend_the_code() {
        let r = rig();
        let long = format!(
            r#"{{"fcm_token":"{}"}}"#,
            "x".repeat(MAX_FCM_TOKEN_CHARS + 1)
        );
        for body in [
            "",
            "not json",
            "[]",
            r#"{}"#,
            r#"{"fcm_token":""}"#,
            r#"{"fcm_token":5}"#,
            "{\"fcm_token\":\"a\\nb\"}",
            long.as_str(),
        ] {
            let e = go(&r, Some(&bearer(&r.code)), body, 1001).unwrap_err();
            assert!(matches!(e, RegisterError::BadRequest(_)), "{body:?}");
        }
        assert!(go(&r, Some(&bearer(&r.code)), BODY, 1002).is_ok());
    }

    #[test]
    fn answers_never_contain_the_code_or_the_tokens_sent() {
        let r = rig();
        let body = r#"{"fcm_token":"SECRET-FCM-TOKEN"}"#;
        let wrong = "Bearer WRONG-CODE-VALUE";
        let mut texts = vec![];
        texts.push(response(&go(&r, Some(wrong), body, 1001)).1.to_string());
        texts.push(
            response(&go(&r, Some(&bearer(&r.code)), "nope", 1001))
                .1
                .to_string(),
        );
        for _ in 0..8 {
            texts.push(response(&go(&r, Some(wrong), body, 1001)).1.to_string());
        }
        for t in texts {
            for secret in [r.code.as_str(), "WRONG-CODE-VALUE", "SECRET-FCM-TOKEN"] {
                assert!(!t.contains(secret), "{t}");
            }
        }
    }

    #[test]
    fn registering_again_with_a_new_code_replaces_the_device() {
        let r = rig();
        let first = go(&r, Some(&bearer(&r.code)), BODY, 1001).unwrap();
        let (code2, e) = new_enrollment(1002);
        r.enr.set(e);
        let second = go(&r, Some(&bearer(&code2)), r#"{"fcm_token":"tok-2"}"#, 1003).unwrap();
        let d = r.dev.device().unwrap();
        assert!(d.ack_token.matches(&second) && !d.ack_token.matches(&first));
        assert_eq!(d.push_token.expose(), "tok-2");
    }

    #[test]
    fn ack_tokens_are_64_hex_and_unique() {
        let a = new_ack_token();
        assert_eq!(a.len(), 64);
        assert!(
            a.bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        );
        assert_ne!(a, new_ack_token());
    }

    #[test]
    fn the_second_contact_registers_and_rotates_their_token_without_touching_the_first() {
        use crate::enroll::{Contact, new_enrollment_for};
        let r = rig();
        let first = go(&r, Some(&format!("Bearer {}", r.code)), BODY, 1001).unwrap();
        let (code2, e2) = new_enrollment_for(Contact::Second, 1002);
        r.enr.set(e2);
        let second = go(
            &r,
            Some(&format!("Bearer {code2}")),
            r#"{"fcm_token":"t2"}"#,
            1003,
        )
        .unwrap();
        assert_ne!(first, second);
        assert!(r.dev.device().unwrap().ack_token.matches(&first));
        assert!(r.dev.second_device().unwrap().ack_token.matches(&second));
        let bearer = format!("Bearer {second}");
        assert_eq!(
            update_token(&r.dev, Some(&bearer), r#"{"fcm_token":"t3"}"#),
            Ok(())
        );
        assert!(
            r.dev
                .device()
                .is_some_and(|d| d.push_token.expose() == "tok-abc")
        );
    }
}
