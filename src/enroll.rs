//! Registration slice (b): the one-time enrolment code and its store (DESIGN-REGISTRATION.md).
//!
//! The operator makes a code (`new_enrollment`), reads it off the VPS once, and the phone presents
//! it to `POST /register` (slice c). The code is 128 random bits, valid 10 minutes, usable once; only
//! its SHA-256 digest is stored and it is compared in constant time (via [`AckToken`]). It is never
//! logged: `Debug` prints `<redacted>`. At most one enrolment is pending (one device per recipient);
//! making a new one replaces the old.

use std::fmt;
use std::sync::Mutex;

use crate::ack::AckToken;

/// How long a code stays valid, in seconds.
pub const ENROLL_TTL_SECS: u64 = 600;

/// A pending enrolment: the digest of the code and when it stops working.
#[derive(Clone)]
pub struct Enrollment {
    code: AckToken,
    pub expires_at: u64,
}

impl fmt::Debug for Enrollment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Enrollment")
            .field("code", &"<redacted>")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

/// Why a code was refused. Callers answer one fixed 401 whatever the reason (it must teach nothing);
/// the reason is for the audit trail only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnrollError {
    /// No pending enrolment, a wrong code, or a code already used.
    Unknown,
    /// The code was right but its 10 minutes are over.
    Expired,
}

/// Where the pending enrolment lives. A PersistAnt-backed store comes later.
pub trait EnrollmentStore: Send + Sync {
    /// Makes `e` the pending enrolment, replacing any earlier one.
    fn set(&self, e: Enrollment);
    /// Checks `presented` against the pending enrolment. A right, unexpired code is accepted once
    /// and removed in the same atomic step; an expired right code is removed and refused; a wrong
    /// code changes nothing (guessing must not burn the real code; the rate limit in slice c stops
    /// guessing).
    fn consume(&self, presented: &str, now: u64) -> Result<(), EnrollError>;
}

/// A new 128-bit code as 32 lowercase hex digits, and the enrolment to store for it. The code is
/// shown to the operator once; nothing else keeps it.
pub fn new_enrollment(now: u64) -> (String, Enrollment) {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).expect("the operating system's random source is unavailable");
    let code: String = b.iter().map(|x| format!("{x:02x}")).collect();
    let e = Enrollment {
        code: AckToken::new(&code),
        expires_at: now + ENROLL_TTL_SECS,
    };
    (code, e)
}

/// In-memory [`EnrollmentStore`], for tests and a single-process service until PersistAnt is wired.
#[derive(Debug, Default)]
pub struct MemoryEnrollments {
    pending: Mutex<Option<Enrollment>>,
}

impl EnrollmentStore for MemoryEnrollments {
    fn set(&self, e: Enrollment) {
        *self.pending.lock().unwrap_or_else(|p| p.into_inner()) = Some(e);
    }

    fn consume(&self, presented: &str, now: u64) -> Result<(), EnrollError> {
        let mut pending = self.pending.lock().unwrap_or_else(|p| p.into_inner());
        let Some(e) = pending.as_ref() else {
            return Err(EnrollError::Unknown);
        };
        if !e.code.matches(presented) {
            return Err(EnrollError::Unknown);
        }
        let expired = now >= e.expires_at;
        *pending = None;
        if expired {
            Err(EnrollError::Expired)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn store_with(now: u64) -> (MemoryEnrollments, String) {
        let s = MemoryEnrollments::default();
        let (code, e) = new_enrollment(now);
        s.set(e);
        (s, code)
    }

    #[test]
    fn a_fresh_code_is_accepted_once() {
        let (s, code) = store_with(1000);
        assert_eq!(s.consume(&code, 1001), Ok(()));
    }

    #[test]
    fn a_reused_code_is_refused() {
        let (s, code) = store_with(1000);
        s.consume(&code, 1001).unwrap();
        assert_eq!(s.consume(&code, 1002), Err(EnrollError::Unknown));
    }

    #[test]
    fn an_expired_code_is_refused_and_stays_refused() {
        let (s, code) = store_with(1000);
        let at = 1000 + ENROLL_TTL_SECS;
        assert_eq!(s.consume(&code, at), Err(EnrollError::Expired));
        assert_eq!(s.consume(&code, 1001), Err(EnrollError::Unknown));
    }

    #[test]
    fn the_last_valid_second_still_works() {
        let (s, code) = store_with(1000);
        assert_eq!(s.consume(&code, 1000 + ENROLL_TTL_SECS - 1), Ok(()));
    }

    #[test]
    fn wrong_codes_are_refused_and_do_not_burn_the_real_one() {
        let (s, code) = store_with(1000);
        for bad in ["", "x", &code[..31], &format!("{code}0"), &"0".repeat(32)] {
            assert_eq!(s.consume(bad, 1001), Err(EnrollError::Unknown), "{bad:?}");
        }
        assert_eq!(s.consume(&code, 1002), Ok(()));
    }

    #[test]
    fn no_pending_enrolment_refuses_everything() {
        let s = MemoryEnrollments::default();
        assert_eq!(s.consume("anything", 1), Err(EnrollError::Unknown));
    }

    #[test]
    fn a_new_enrolment_replaces_the_old_code() {
        let (s, old) = store_with(1000);
        let (new, e) = new_enrollment(1001);
        s.set(e);
        assert_eq!(s.consume(&old, 1002), Err(EnrollError::Unknown));
        assert_eq!(s.consume(&new, 1002), Ok(()));
    }

    #[test]
    fn racing_presentations_accept_exactly_one() {
        let (s, code) = store_with(1000);
        let s = Arc::new(s);
        let handles: Vec<_> = (0..16)
            .map(|_| {
                let (s, code) = (Arc::clone(&s), code.clone());
                std::thread::spawn(move || s.consume(&code, 1001).is_ok())
            })
            .collect();
        let ok = handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .filter(|x| *x)
            .count();
        assert_eq!(ok, 1);
    }

    #[test]
    fn codes_are_32_hex_digits_and_unique_and_never_print() {
        let (a, ea) = new_enrollment(5);
        let (b, _) = new_enrollment(5);
        assert_eq!(a.len(), 32);
        assert!(
            a.bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        );
        assert_ne!(a, b);
        assert_eq!(ea.expires_at, 5 + ENROLL_TTL_SECS);
        assert!(!format!("{ea:?}").contains(&a));
    }
}
