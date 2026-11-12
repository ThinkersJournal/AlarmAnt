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
use crate::enroll::EnrollmentStore;
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
    /// Makes `d` the device, replacing any earlier one.
    fn set(&self, d: Device);
}

/// In-memory [`DeviceStore`].
#[derive(Debug, Default)]
pub struct MemoryDevices {
    device: Mutex<Option<Device>>,
}

impl DeviceRegistry for MemoryDevices {
    fn device(&self) -> Option<Device> {
        self.device
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
}

impl DeviceStore for MemoryDevices {
    fn set(&self, d: Device) {
        *self.device.lock().unwrap_or_else(|p| p.into_inner()) = Some(d);
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
    let accepted = authorization
        .and_then(bearer_token)
        .is_some_and(|code| enrollments.consume(code, now).is_ok());
    if !accepted {
        return Err(if limiter.record_failure(now) {
            RegisterError::TooManyFailures
        } else {
            RegisterError::Unauthorized
        });
    }
    let token = new_ack_token();
    devices.set(Device {
        push_token: DeviceToken::new(fcm),
        ack_token: AckToken::new(&token),
    });
    Ok(token)
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
}
