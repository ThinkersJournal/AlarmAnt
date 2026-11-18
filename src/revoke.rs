//! Registration slice (e): revocation (DESIGN-REGISTRATION.md section 4).
//!
//! `revoke` deletes the device record on the server and voids the ack token of every alert still
//! in flight, so a lost phone can neither receive new alerts nor stand down an alert and stop its
//! escalation: those alerts escalate on their normal timer. Constraints from the patent reading
//! (PM 2026-11-11): server-side record delete only; no re-validation or restore step for the old
//! device; the phone never invalidates itself and is told nothing; no transaction roll-back. There
//! is deliberately no function here that undoes a revocation: a new phone enrols again.
//! Nothing here logs, and nothing carries a token.

use crate::ack::AlertStore;
use crate::enroll::Contact;
use crate::register::DeviceStore;

/// What one revocation did, for the operator's screen and the audit event.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Revoked {
    /// A device record existed and was deleted.
    pub had_device: bool,
    /// In-flight alerts that stopped accepting the old ack token.
    pub alerts_voided: usize,
}

/// Revokes the registered device: deletes its record, then voids the ack token of every alert
/// still in flight. The record goes first, so no new alert can be raised for it while the old
/// ones are being voided ([`crate::raise::raise_authorised`] rechecks for one already mid-raise).
/// Revoking with no device registered does nothing.
pub fn revoke(devices: &dyn DeviceStore, alerts: &dyn AlertStore) -> Revoked {
    revoke_contact(devices, alerts, Contact::First)
}

/// [`revoke`] for a chosen contact: only that contact's device goes, and only its token is voided.
pub fn revoke_contact(
    devices: &dyn DeviceStore,
    alerts: &dyn AlertStore,
    contact: Contact,
) -> Revoked {
    match devices.take_for(contact) {
        Some(device) => Revoked {
            had_device: true,
            alerts_voided: alerts.void_ack_tokens(&device.ack_token),
        },
        None => Revoked::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AlertState;
    use crate::ack::{AckOutcome, AckToken, AlertRecord, MemoryStore, acknowledge};
    use crate::fcm::DeviceToken;
    use crate::raise::{Device, DeviceRegistry};
    use crate::register::MemoryDevices;

    fn rig() -> (MemoryDevices, MemoryStore) {
        let devices = MemoryDevices::default();
        devices.set(Device {
            push_token: DeviceToken::new("fcm"),
            ack_token: AckToken::new("old-ack"),
        });
        let alerts = MemoryStore::default();
        alerts.insert(AlertRecord::sent("a-1", 100, AckToken::new("old-ack")));
        (devices, alerts)
    }

    #[test]
    fn revoking_deletes_the_device_record() {
        let (d, a) = rig();
        let r = revoke(&d, &a);
        assert!(r.had_device);
        assert!(d.device().is_none());
    }

    #[test]
    fn an_in_flight_alert_no_longer_accepts_the_old_token_and_stays_sent() {
        let (d, a) = rig();
        assert_eq!(
            acknowledge(&a, "a-1", Some("Bearer old-ack"), 150),
            AckOutcome::Acked,
            "control: before revocation the old token works"
        );
        let (d2, a2) = rig();
        let r = revoke(&d2, &a2);
        assert_eq!(r.alerts_voided, 1);
        assert_eq!(
            acknowledge(&a2, "a-1", Some("Bearer old-ack"), 150),
            AckOutcome::Unauthorized
        );
        assert_eq!(a2.get("a-1").unwrap().state, AlertState::Sent);
        drop((d, a));
    }

    #[test]
    fn no_presented_value_acks_a_voided_alert() {
        let (d, a) = rig();
        revoke(&d, &a);
        for t in [
            "",
            "old-ack",
            "0000000000000000000000000000000000000000000000000000000000000000",
        ] {
            let bearer = format!("Bearer {t}");
            assert_eq!(
                acknowledge(&a, "a-1", Some(&bearer), 150),
                AckOutcome::Unauthorized,
                "{t:?}"
            );
        }
    }

    #[test]
    fn an_already_acknowledged_alert_is_left_alone() {
        let (d, a) = rig();
        acknowledge(&a, "a-1", Some("Bearer old-ack"), 150);
        let r = revoke(&d, &a);
        assert_eq!(r.alerts_voided, 0);
        let rec = a.get("a-1").unwrap();
        assert_eq!(rec.state, AlertState::Acked);
        assert_eq!(rec.acked_at, Some(150));
    }

    #[test]
    fn alerts_of_another_device_are_untouched_and_a_second_revoke_is_harmless() {
        let (d, a) = rig();
        a.insert(AlertRecord::sent("a-2", 101, AckToken::new("other-ack")));
        let first = revoke(&d, &a);
        assert_eq!(first.alerts_voided, 1);
        assert_eq!(
            acknowledge(&a, "a-2", Some("Bearer other-ack"), 160),
            AckOutcome::Acked
        );
        let again = revoke(&d, &a);
        assert_eq!(again, Revoked::default());
    }

    #[test]
    fn the_old_token_cannot_rotate_a_push_token_after_revocation() {
        let (d, a) = rig();
        revoke(&d, &a);
        assert!(!d.replace_push_token("old-ack", DeviceToken::new("new")));
        assert!(d.device().is_none());
    }
}
