//! AlarmAnt: an emergency alert that cannot be ignored.
//!
//! This release holds the A5 escalation state machine only (see `DESIGN-A5.md`).
//! It is pure logic: no timers, HTTP, retries or provider code. The caller owns
//! the clock and calls [`Alert::on`] when the timeout elapses or an acknowledgement
//! arrives; sending goes through the [`AlertChannel`] trait.
#![forbid(unsafe_code)]

pub mod ack;
pub mod admin;
pub mod auth;
pub mod enroll;
pub mod escalate;
pub mod fcm;
pub mod push;
pub mod raise;
pub mod register;
pub mod revoke;
pub mod routes;
pub mod selftest;
pub mod wire;

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

/// A channel failure. The state machine never retries; `retryable` is for the adapter's caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelError {
    pub retryable: bool,
    pub message: String,
}

/// How the later tiers are reached (ruling 389). Adapters implement this: the second contact's app
/// push, and the PC alarm. Neither call carries a phone number, and nothing here reads a reply.
pub trait AlertChannel {
    /// Push the alert to the second contact's app.
    fn second_app(&self, alert_id: &str) -> Result<(), ChannelError>;
    /// Raise the PC alarm for the alert.
    fn pc_alarm(&self, alert_id: &str) -> Result<(), ChannelError>;
}

/// What one [`Alert::on`] call did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    /// Audit events to record, in order.
    pub audit: Vec<AuditEvent>,
    /// Channel failures from this step. The state still advanced.
    pub send_errors: Vec<ChannelError>,
}

/// How far down the flat chain an alert has gone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Sent to the first contact's app.
    First,
    /// The second contact's app has been pushed.
    SecondApp,
    /// The PC alarm has been raised.
    PcAlarm,
}

/// One alert's escalation state.
#[derive(Debug)]
pub struct Alert {
    id: String,
    state: AlertState,
    stage: Stage,
}

impl Alert {
    /// A new alert already pushed to the first contact.
    pub fn new(id: impl Into<String>) -> Self {
        Alert {
            id: id.into(),
            state: AlertState::Sent,
            stage: Stage::First,
        }
    }

    /// Rebuild an alert from stored parts (after a restart).
    pub fn restore(id: impl Into<String>, state: AlertState, stage: Stage) -> Self {
        Alert {
            id: id.into(),
            state,
            stage,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn stage(&self) -> Stage {
        self.stage
    }

    pub fn state(&self) -> AlertState {
        self.state
    }

    /// Apply an event. The second contact's app is pushed on the first timeout and the PC alarm
    /// raised on the second; the third ends in [`AlertState::Unresolved`]. An
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
                    self.stage = Stage::SecondApp;
                    self.state = AlertState::Escalated;
                    out.audit.push(AuditEvent::Escalated);
                    if let Err(e) = channel.second_app(&self.id) {
                        out.send_errors.push(e);
                    }
                }
                Stage::SecondApp => {
                    self.stage = Stage::PcAlarm;
                    if let Err(e) = channel.pc_alarm(&self.id) {
                        out.send_errors.push(e);
                    }
                }
                Stage::PcAlarm => {
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
        fn second_app(&self, _: &str) -> Result<(), ChannelError> {
            self.calls.borrow_mut().push("second_app");
            self.result()
        }
        fn pc_alarm(&self, _: &str) -> Result<(), ChannelError> {
            self.calls.borrow_mut().push("pc_alarm");
            self.result()
        }
    }

    fn alert() -> Alert {
        Alert::new("a1")
    }

    #[test]
    fn full_chain_second_app_then_pc_alarm_then_unresolved() {
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
        assert_eq!(*ch.calls.borrow(), vec!["second_app", "pc_alarm"]);
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
            vec!["second_app"],
            "no retry inside the state machine"
        );
    }

    #[test]
    fn audit_names() {
        assert_eq!(AuditEvent::Escalated.name(), "alert.escalated");
        assert_eq!(AuditEvent::Acked.name(), "alert.acked");
        assert_eq!(AuditEvent::Unresolved.name(), "alert.unresolved");
    }
}
