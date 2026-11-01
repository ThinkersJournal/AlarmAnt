//! AlarmAnt: an emergency alert that cannot be ignored.
//!
//! This release holds the A5 escalation state machine only (see `DESIGN-A5.md`).
//! It is pure logic: no timers, HTTP, retries or provider code. The caller owns
//! the clock and calls [`Alert::on`] when the timeout elapses or an acknowledgement
//! arrives; sending goes through the [`AlertChannel`] trait.
#![forbid(unsafe_code)]

pub mod ack;
pub mod fcm;
pub mod raise;
pub mod selftest;

use std::fmt;

/// Where an alert stands. Flat chain: person 1, then person 2, no third tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertState {
    /// Sent to person 1 (A2); waiting for an acknowledgement.
    Sent,
    /// Person 2 has been texted or called; waiting for an acknowledgement.
    Escalated,
    /// Someone acknowledged. Final.
    Acked,
    /// Person 2 did not acknowledge either. Only an acknowledgement ends it.
    Unresolved,
}

/// What happens to an alert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertEvent {
    /// The configured number of minutes passed without an acknowledgement.
    TimeoutElapsed,
    /// Anyone acknowledged the alert.
    Ack,
}

/// Audit events the caller must record (AuditAnt).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditEvent {
    Escalated,
    Acked,
    Unresolved,
}

impl AuditEvent {
    /// The event name written to the audit trail.
    pub fn name(self) -> &'static str {
        match self {
            AuditEvent::Escalated => "alert.escalated",
            AuditEvent::Acked => "alert.acked",
            AuditEvent::Unresolved => "alert.unresolved",
        }
    }
}

/// Person 2's phone number. Personal data: `Debug` and `Display` never show it.
#[derive(Clone, PartialEq, Eq)]
pub struct Contact(String);

impl Contact {
    pub fn new(number: impl Into<String>) -> Self {
        Contact(number.into())
    }

    /// The raw number, for the channel adapter only.
    pub fn number(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Contact {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Contact(<redacted>)")
    }
}

impl fmt::Display for Contact {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

/// A channel failure. The state machine never retries; `retryable` is for the adapter's caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelError {
    pub retryable: bool,
    pub message: String,
}

/// How person 2 is reached. Adapters (SMSKit or a provider) implement this later.
pub trait AlertChannel {
    /// Send the alert to `to` by SMS.
    fn sms(&self, to: &Contact, alert_id: &str) -> Result<(), ChannelError>;
    /// Place a voice call to `to`.
    fn voice(&self, to: &Contact, alert_id: &str) -> Result<(), ChannelError>;
}

/// What one [`Alert::on`] call did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    /// Audit events to record, in order.
    pub audit: Vec<AuditEvent>,
    /// Channel failures from this step. The state still advanced.
    pub send_errors: Vec<ChannelError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    First,
    Sms,
    Voice,
}

/// One alert's escalation state.
#[derive(Debug)]
pub struct Alert {
    id: String,
    person2: Contact,
    state: AlertState,
    stage: Stage,
}

impl Alert {
    /// A new alert already sent to person 1.
    pub fn new(id: impl Into<String>, person2: Contact) -> Self {
        Alert {
            id: id.into(),
            person2,
            state: AlertState::Sent,
            stage: Stage::First,
        }
    }

    pub fn state(&self) -> AlertState {
        self.state
    }

    /// Apply an event. Person 2 is texted on the first timeout and called on
    /// the second; the third ends in [`AlertState::Unresolved`]. An
    /// acknowledgement from anyone ends the alert from any state, and later
    /// events on an acknowledged alert do nothing.
    pub fn on(&mut self, event: AlertEvent, channel: &dyn AlertChannel) -> Outcome {
        let mut out = Outcome::default();
        match (self.state, event) {
            (AlertState::Acked, _) => {}
            (_, AlertEvent::Ack) => {
                self.state = AlertState::Acked;
                out.audit.push(AuditEvent::Acked);
            }
            (AlertState::Unresolved, AlertEvent::TimeoutElapsed) => {}
            (_, AlertEvent::TimeoutElapsed) => match self.stage {
                Stage::First => {
                    self.stage = Stage::Sms;
                    self.state = AlertState::Escalated;
                    out.audit.push(AuditEvent::Escalated);
                    if let Err(e) = channel.sms(&self.person2, &self.id) {
                        out.send_errors.push(e);
                    }
                }
                Stage::Sms => {
                    self.stage = Stage::Voice;
                    if let Err(e) = channel.voice(&self.person2, &self.id) {
                        out.send_errors.push(e);
                    }
                }
                Stage::Voice => {
                    self.state = AlertState::Unresolved;
                    out.audit.push(AuditEvent::Unresolved);
                }
            },
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[derive(Default)]
    struct Fake {
        calls: RefCell<Vec<&'static str>>,
        fail: bool,
    }

    impl Fake {
        fn result(&self) -> Result<(), ChannelError> {
            if self.fail {
                Err(ChannelError {
                    retryable: true,
                    message: "down".into(),
                })
            } else {
                Ok(())
            }
        }
    }

    impl AlertChannel for Fake {
        fn sms(&self, _: &Contact, _: &str) -> Result<(), ChannelError> {
            self.calls.borrow_mut().push("sms");
            self.result()
        }
        fn voice(&self, _: &Contact, _: &str) -> Result<(), ChannelError> {
            self.calls.borrow_mut().push("voice");
            self.result()
        }
    }

    fn alert() -> Alert {
        Alert::new("a1", Contact::new("+15550100"))
    }

    #[test]
    fn full_chain_sms_then_voice_then_unresolved() {
        let (mut a, ch) = (alert(), Fake::default());
        assert_eq!(
            a.on(AlertEvent::TimeoutElapsed, &ch).audit,
            vec![AuditEvent::Escalated]
        );
        assert_eq!(a.state(), AlertState::Escalated);
        assert!(a.on(AlertEvent::TimeoutElapsed, &ch).audit.is_empty());
        assert_eq!(a.state(), AlertState::Escalated);
        assert_eq!(
            a.on(AlertEvent::TimeoutElapsed, &ch).audit,
            vec![AuditEvent::Unresolved]
        );
        assert_eq!(a.state(), AlertState::Unresolved);
        assert_eq!(*ch.calls.borrow(), vec!["sms", "voice"]);
        // No third tier: further timeouts do nothing.
        assert_eq!(a.on(AlertEvent::TimeoutElapsed, &ch), Outcome::default());
        assert_eq!(ch.calls.borrow().len(), 2);
    }

    #[test]
    fn ack_before_timeout_sends_nothing() {
        let (mut a, ch) = (alert(), Fake::default());
        assert_eq!(a.on(AlertEvent::Ack, &ch).audit, vec![AuditEvent::Acked]);
        assert_eq!(a.state(), AlertState::Acked);
        assert_eq!(a.on(AlertEvent::TimeoutElapsed, &ch), Outcome::default());
        assert!(ch.calls.borrow().is_empty());
    }

    #[test]
    fn ack_ends_alert_from_every_state() {
        for timeouts in 0..=3 {
            let (mut a, ch) = (alert(), Fake::default());
            for _ in 0..timeouts {
                a.on(AlertEvent::TimeoutElapsed, &ch);
            }
            a.on(AlertEvent::Ack, &ch);
            assert_eq!(a.state(), AlertState::Acked, "after {timeouts} timeouts");
        }
    }

    #[test]
    fn second_ack_is_a_no_op() {
        let (mut a, ch) = (alert(), Fake::default());
        a.on(AlertEvent::Ack, &ch);
        assert_eq!(a.on(AlertEvent::Ack, &ch), Outcome::default());
    }

    #[test]
    fn send_failure_is_reported_and_state_still_advances() {
        let (mut a, ch) = (
            alert(),
            Fake {
                fail: true,
                ..Fake::default()
            },
        );
        let out = a.on(AlertEvent::TimeoutElapsed, &ch);
        assert_eq!(out.send_errors.len(), 1);
        assert!(out.send_errors[0].retryable);
        assert_eq!(a.state(), AlertState::Escalated);
        assert_eq!(
            *ch.calls.borrow(),
            vec!["sms"],
            "no retry inside the state machine"
        );
    }

    #[test]
    fn contact_never_prints_the_number() {
        let c = Contact::new("+15550100");
        assert!(!format!("{c:?} {c}").contains("5550100"));
        assert!(!format!("{:?}", alert()).contains("5550100"));
        assert_eq!(c.number(), "+15550100");
    }

    #[test]
    fn audit_names() {
        assert_eq!(AuditEvent::Escalated.name(), "alert.escalated");
        assert_eq!(AuditEvent::Acked.name(), "alert.acked");
        assert_eq!(AuditEvent::Unresolved.name(), "alert.unresolved");
    }
}
