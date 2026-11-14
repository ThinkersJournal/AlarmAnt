//! Registration slice (b): the one-time enrolment code and its store (DESIGN-REGISTRATION.md).
//!
//! The operator makes a code (`new_enrollment`), reads it off the VPS once, and the phone presents
//! it to `POST /register` (slice c). The code is 128 random bits, valid 10 minutes, usable once; only
//! its SHA-256 digest is stored and it is compared in constant time (via [`AckToken`]). It is never
//! logged: `Debug` prints `<redacted>`. At most one enrolment is pending per contact (one device per recipient);
//! making a new one for the same contact replaces the old, and leaves the other contact's alone.

use std::fmt;
use std::sync::Mutex;

use crate::ack::AckToken;

/// How long a code stays valid, in seconds.
pub const ENROLL_TTL_SECS: u64 = 600;

/// Which recipient a device or an enrolment is for (ruling 389): the first contact's app is pushed
/// first, the second contact's after one timer step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Contact {
    First,
    Second,
}

impl Contact {
    pub(crate) fn index(self) -> usize {
        match self {
            Contact::First => 0,
            Contact::Second => 1,
        }
    }
}

/// A pending enrolment: the digest of the code, who it is for, and when it stops working.
#[derive(Clone)]
pub struct Enrollment {
    code: AckToken,
    pub contact: Contact,
    pub expires_at: u64,
}

impl Enrollment {
    /// The stored form: digest, contact and expiry. The code itself is not kept anywhere.
    pub(crate) fn to_value(&self) -> serde_json::Value {
        serde_json::json!({
            "code_digest": self.code.digest_hex(),
            "contact": match self.contact { Contact::First => "first", Contact::Second => "second" },
            "expires_at": self.expires_at,
        })
    }

    pub(crate) fn from_value(v: &serde_json::Value) -> Option<Enrollment> {
        Some(Enrollment {
            code: AckToken::from_digest_hex(v.get("code_digest")?.as_str()?)?,
            contact: match v.get("contact")?.as_str()? {
                "first" => Contact::First,
                "second" => Contact::Second,
                _ => return None,
            },
            expires_at: v.get("expires_at")?.as_u64()?,
        })
    }
}

impl fmt::Debug for Enrollment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Enrollment")
            .field("code", &"<redacted>")
            .field("contact", &self.contact)
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
    /// Makes `e` the pending enrolment for its contact, replacing any earlier one for that contact.
    fn set(&self, e: Enrollment);
    /// Checks `presented` against the pending enrolments. A right, unexpired code is accepted once,
    /// answers who it was for, and is removed in the same atomic step; an expired right code is removed and refused; a wrong
    /// code changes nothing (guessing must not burn the real code; the rate limit in slice c stops
    /// guessing).
    fn consume(&self, presented: &str, now: u64) -> Result<Contact, EnrollError>;
}

/// A new 128-bit code as 32 lowercase hex digits, and the enrolment to store for it. The code is
/// shown to the operator once; nothing else keeps it.
pub fn new_enrollment(now: u64) -> (String, Enrollment) {
    new_enrollment_for(Contact::First, now)
}

/// [`new_enrollment`] for a chosen contact.
pub fn new_enrollment_for(contact: Contact, now: u64) -> (String, Enrollment) {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).expect("the operating system's random source is unavailable");
    let code: String = b.iter().map(|x| format!("{x:02x}")).collect();
    let e = Enrollment {
        code: AckToken::new(&code),
        contact,
        expires_at: now + ENROLL_TTL_SECS,
    };
    (code, e)
}

/// In-memory [`EnrollmentStore`], for tests and a single-process service until PersistAnt is wired.
#[derive(Debug, Default)]
pub struct MemoryEnrollments {
    pending: Mutex<[Option<Enrollment>; 2]>,
}

impl MemoryEnrollments {
    pub(crate) fn snapshot(&self) -> Vec<Enrollment> {
        let g = self.pending.lock().unwrap_or_else(|p| p.into_inner());
        g.iter().flatten().cloned().collect()
    }
}

impl EnrollmentStore for MemoryEnrollments {
    fn set(&self, e: Enrollment) {
        let i = e.contact.index();
        self.pending.lock().unwrap_or_else(|p| p.into_inner())[i] = Some(e);
    }

    fn consume(&self, presented: &str, now: u64) -> Result<Contact, EnrollError> {
        let mut pending = self.pending.lock().unwrap_or_else(|p| p.into_inner());
        // Both slots are always checked, so timing does not say which contact a code was for.
        let mut hit = None;
        for (i, slot) in pending.iter().enumerate() {
            if slot.as_ref().is_some_and(|e| e.code.matches(presented)) {
                hit = Some(i);
            }
        }
        let Some(i) = hit else {
            return Err(EnrollError::Unknown);
        };
        let e = pending[i].take().expect("slot was just matched");
        if now >= e.expires_at {
            Err(EnrollError::Expired)
        } else {
            Ok(e.contact)
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
        assert_eq!(s.consume(&code, 1001), Ok(Contact::First));
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
        assert_eq!(
            s.consume(&code, 1000 + ENROLL_TTL_SECS - 1),
            Ok(Contact::First)
        );
    }

    #[test]
    fn wrong_codes_are_refused_and_do_not_burn_the_real_one() {
        let (s, code) = store_with(1000);
        for bad in ["", "x", &code[..31], &format!("{code}0"), &"0".repeat(32)] {
            assert_eq!(s.consume(bad, 1001), Err(EnrollError::Unknown), "{bad:?}");
        }
        assert_eq!(s.consume(&code, 1002), Ok(Contact::First));
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
        assert_eq!(s.consume(&new, 1002), Ok(Contact::First));
    }

    #[test]
    fn each_contact_has_its_own_pending_code_and_codes_name_their_contact() {
        let s = MemoryEnrollments::default();
        let (c1, e1) = new_enrollment_for(Contact::First, 1000);
        let (c2, e2) = new_enrollment_for(Contact::Second, 1000);
        s.set(e1);
        s.set(e2);
        assert_eq!(s.consume(&c2, 1001), Ok(Contact::Second));
        assert_eq!(s.consume(&c1, 1001), Ok(Contact::First));
    }

    #[test]
    fn a_new_code_for_one_contact_leaves_the_other_contacts_code_alone() {
        let s = MemoryEnrollments::default();
        let (c1, e1) = new_enrollment_for(Contact::First, 1000);
        let (c2, e2) = new_enrollment_for(Contact::Second, 1000);
        s.set(e1);
        s.set(e2);
        let (c2b, e2b) = new_enrollment_for(Contact::Second, 1001);
        s.set(e2b);
        assert_eq!(s.consume(&c2, 1002), Err(EnrollError::Unknown));
        assert_eq!(s.consume(&c1, 1002), Ok(Contact::First));
        assert_eq!(s.consume(&c2b, 1002), Ok(Contact::Second));
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
