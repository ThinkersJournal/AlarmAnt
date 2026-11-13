//! Registration slice (f): the operator commands `alarmant enroll --contact first|second` and
//! `alarmant revoke --contact first|second` (DESIGN-REGISTRATION.md).
//!
//! This module is the command logic, not a process: [`parse`] turns arguments into a [`Command`]
//! and [`run`] carries it out against the stores and returns the text for the operator's screen.
//! The binary that wires real (PersistAnt) stores comes with slice (g).
//!
//! The code is shown on the operator's screen once and nowhere else: [`run`] returns it in the
//! text and keeps nothing; no function here logs. Revocation says only what the server did; it
//! never contacts or tells the phone, and there is no command that undoes it.
//!
//! Each contact has their own pending code and device slot; a command about one never touches the
//! other.

use crate::ack::AlertStore;
pub use crate::enroll::Contact;
use crate::enroll::{ENROLL_TTL_SECS, EnrollmentStore, new_enrollment_for};
use crate::register::DeviceStore;
use crate::revoke::revoke_contact;

/// An operator command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Enroll(Contact),
    Revoke(Contact),
}

/// Why a command line or command was refused; the text is for the operator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdminError {
    Usage(&'static str),
}

impl AdminError {
    pub fn message(&self) -> &'static str {
        match self {
            AdminError::Usage(m) => m,
        }
    }
}

const USAGE: &str = "usage: alarmant enroll|revoke --contact first|second";

/// Parses the arguments after the program name. `--contact` is required: which recipient a code
/// or a revocation is for is never guessed.
pub fn parse(args: &[String]) -> Result<Command, AdminError> {
    let [verb, flag, who] = args else {
        return Err(AdminError::Usage(USAGE));
    };
    if flag != "--contact" {
        return Err(AdminError::Usage(USAGE));
    }
    let contact = match who.as_str() {
        "first" => Contact::First,
        "second" => Contact::Second,
        _ => return Err(AdminError::Usage(USAGE)),
    };
    match verb.as_str() {
        "enroll" => Ok(Command::Enroll(contact)),
        "revoke" => Ok(Command::Revoke(contact)),
        _ => Err(AdminError::Usage(USAGE)),
    }
}

/// Carries out `command` and returns the text to show the operator.
pub fn run(
    command: Command,
    enrollments: &dyn EnrollmentStore,
    devices: &dyn DeviceStore,
    alerts: &dyn AlertStore,
    now: u64,
) -> Result<String, AdminError> {
    match command {
        Command::Enroll(contact) => {
            let (code, enrollment) = new_enrollment_for(contact, now);
            enrollments.set(enrollment);
            Ok(format!(
                "Enrolment code (shown once, valid {} minutes, usable once):\n{code}\n",
                ENROLL_TTL_SECS / 60
            ))
        }
        Command::Revoke(contact) => {
            let r = revoke_contact(devices, alerts, contact);
            Ok(if r.had_device {
                format!(
                    "Device removed. {} alert(s) in flight no longer accept its token.\n",
                    r.alerts_voided
                )
            } else {
                "No device was registered; nothing changed.\n".to_owned()
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ack::{AckToken, AlertRecord, MemoryStore};
    use crate::enroll::MemoryEnrollments;
    use crate::fcm::DeviceToken;
    use crate::raise::{Device, DeviceRegistry};
    use crate::register::MemoryDevices;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    fn rig() -> (MemoryEnrollments, MemoryDevices, MemoryStore) {
        let devices = MemoryDevices::default();
        devices.set(Device {
            push_token: DeviceToken::new("fcm"),
            ack_token: AckToken::new("ack"),
        });
        let alerts = MemoryStore::default();
        alerts.insert(AlertRecord::sent("a-1", 100, AckToken::new("ack")));
        (MemoryEnrollments::default(), devices, alerts)
    }

    #[test]
    fn parses_enroll_and_revoke_for_each_contact() {
        assert_eq!(
            parse(&args(&["enroll", "--contact", "first"])),
            Ok(Command::Enroll(Contact::First))
        );
        assert_eq!(
            parse(&args(&["revoke", "--contact", "second"])),
            Ok(Command::Revoke(Contact::Second))
        );
    }

    #[test]
    fn every_malformed_command_line_is_a_usage_error() {
        for bad in [
            &[][..],
            &["enroll"],
            &["enroll", "--contact"],
            &["enroll", "first"],
            &["enroll", "--contact", "third"],
            &["enroll", "--contact", "First"],
            &["restore", "--contact", "first"],
            &["enroll", "--contact", "first", "extra"],
        ] {
            assert!(
                matches!(parse(&args(bad)), Err(AdminError::Usage(_))),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn there_is_no_command_that_undoes_a_revocation() {
        for verb in ["restore", "undo", "unrevoke", "reinstate"] {
            assert!(parse(&args(&[verb, "--contact", "first"])).is_err());
        }
    }

    #[test]
    fn enroll_prints_a_code_the_phone_can_spend_exactly_once() {
        let (e, d, a) = rig();
        let out = run(Command::Enroll(Contact::First), &e, &d, &a, 1000).unwrap();
        let code = out
            .lines()
            .find(|l| l.len() == 32 && l.chars().all(|c| c.is_ascii_hexdigit()))
            .expect("the code is on its own line");
        assert!(out.contains("10 minutes"));
        use crate::enroll::EnrollmentStore;
        assert!(e.consume(code, 1001).is_ok());
        assert!(e.consume(code, 1002).is_err());
    }

    #[test]
    fn a_second_enroll_replaces_the_first_code() {
        let (e, d, a) = rig();
        let first = run(Command::Enroll(Contact::First), &e, &d, &a, 1000).unwrap();
        let second = run(Command::Enroll(Contact::First), &e, &d, &a, 1001).unwrap();
        assert_ne!(first, second);
        let code_of = |t: &str| {
            t.lines()
                .find(|l| l.len() == 32)
                .map(str::to_owned)
                .unwrap()
        };
        use crate::enroll::EnrollmentStore;
        assert!(e.consume(&code_of(&first), 1002).is_err());
        assert!(e.consume(&code_of(&second), 1002).is_ok());
    }

    #[test]
    fn revoke_removes_the_device_and_reports_the_voided_alerts() {
        let (e, d, a) = rig();
        let out = run(Command::Revoke(Contact::First), &e, &d, &a, 1000).unwrap();
        assert!(d.device().is_none());
        assert!(out.contains("Device removed"));
        assert!(out.contains("1 alert(s)"));
    }

    #[test]
    fn revoke_with_no_device_says_nothing_changed() {
        let (e, _, a) = rig();
        let empty = MemoryDevices::default();
        let out = run(Command::Revoke(Contact::First), &e, &empty, &a, 1000).unwrap();
        assert!(out.contains("nothing changed"));
    }

    #[test]
    fn enroll_second_makes_a_code_that_registers_the_second_slot_only() {
        use crate::register::{FailureLimiter, register};
        let (e, d, a) = rig();
        let out = run(Command::Enroll(Contact::Second), &e, &d, &a, 1000).unwrap();
        let code = out.lines().find(|l| l.len() == 32).unwrap();
        let lim = FailureLimiter::default();
        register(
            &e,
            &d,
            &lim,
            Some(&format!("Bearer {code}")),
            r#"{"fcm_token":"t2"}"#,
            1001,
        )
        .unwrap();
        assert!(d.second_device().is_some());
        assert!(
            d.device().unwrap().ack_token.matches("ack"),
            "first is untouched"
        );
    }

    #[test]
    fn revoke_second_removes_only_the_second_device_and_voids_only_its_alerts() {
        let (e, d, a) = rig();
        d.set_for(
            Contact::Second,
            Device {
                push_token: DeviceToken::new("fcm2"),
                ack_token: AckToken::new("ack2"),
            },
        );
        a.insert(AlertRecord::sent("a-2", 100, AckToken::new("ack2")));
        let out = run(Command::Revoke(Contact::Second), &e, &d, &a, 1000).unwrap();
        assert!(out.contains("1 alert(s)"));
        assert!(d.second_device().is_none());
        assert!(d.device().is_some());
        assert!(a.get("a-1").unwrap().ack_token.matches("ack"));
    }
}
