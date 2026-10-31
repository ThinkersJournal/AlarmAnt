//! A2: the FCM HTTP v1 alert message and the seams the sender is built on.
//!
//! Pure data and traits; no network, clock or file access here. The HTTP call (HTTPAnt), the
//! service-account sign-in and the secret store plug in behind [`PushTransport`] and
//! [`SecretSource`] (see `DESIGN-A2.md`).

use std::fmt;
use std::future::Future;

use serde_json::{Value, json};

/// How long Google's push service may hold an undelivered alert, in seconds. An alarm for an
/// incident that is ten minutes old is worse than none: the escalation timer (A5) has moved on.
pub const TTL_SECONDS: u32 = 600;

/// A phone's FCM registration token. Identifies a device, so `Debug` and `Display` never show it.
#[derive(Clone, PartialEq, Eq)]
pub struct DeviceToken(String);

impl DeviceToken {
    pub fn new(token: impl Into<String>) -> Self {
        DeviceToken(token.into())
    }

    /// The raw token, for the transport only.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for DeviceToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DeviceToken(<redacted>)")
    }
}

impl fmt::Display for DeviceToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

/// What the phone shows and rings for. The app reads exactly these three fields and parses nothing
/// else (`AlarmMessagingService`, PATENT-SWEEP constraint 8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlertMessage {
    pub alert_id: String,
    pub title: String,
    pub body: String,
}

/// The JSON body of `POST https://fcm.googleapis.com/v1/projects/{project}/messages:send`.
///
/// A data message with `priority: HIGH` and no `notification` block: with a `notification` block
/// the system shows it itself and the app's alarm code never runs.
pub fn request_body(token: &DeviceToken, alert: &AlertMessage) -> Value {
    json!({
        "message": {
            "token": token.expose(),
            "data": {
                "alert_id": alert.alert_id,
                "title": alert.title,
                "body": alert.body,
            },
            "android": {
                "priority": "HIGH",
                "ttl": format!("{TTL_SECONDS}s"),
            },
        }
    })
}

/// The send URL for a Firebase project.
pub fn send_url(project_id: &str) -> String {
    format!("https://fcm.googleapis.com/v1/projects/{project_id}/messages:send")
}

/// Why a push failed. The sender decides what to do with it; the transport only classifies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PushError {
    /// Try again later: a network failure, a 5xx, or a 429.
    Transient(String),
    /// The token is no longer valid (FCM `UNREGISTERED`): drop it, do not retry.
    TokenGone,
    /// Anything else (bad request, auth failure): retrying will not help.
    Permanent(String),
}

/// What the push service said about an accepted message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushReceipt {
    /// FCM's message name (`projects/{p}/messages/{id}`).
    pub message_name: String,
}

/// Sends one alert to one device. The HTTPAnt-backed implementation comes with the sender; tests
/// use a fake.
pub trait PushTransport {
    fn send(
        &self,
        token: &DeviceToken,
        alert: &AlertMessage,
    ) -> impl Future<Output = Result<PushReceipt, PushError>> + Send;
}

/// Where the Google service-account key comes from. The key is the one secret the sender holds:
/// implementations read it on demand and the caller never logs or stores it. A file implementation
/// comes first; SecretAnt takes over behind the same trait when it ships.
pub trait SecretSource {
    /// The service-account JSON. Fails with a message that never contains the key.
    fn service_account_json(&self) -> Result<String, SecretError>;
}

/// A secret could not be read. The message names the source (a path), never the content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretError(pub String);

#[cfg(test)]
mod tests {
    use super::*;

    fn alert() -> AlertMessage {
        AlertMessage {
            alert_id: "a-1".into(),
            title: "DB down".into(),
            body: "prod-1 unreachable".into(),
        }
    }

    #[test]
    fn body_is_a_high_priority_data_message_without_a_notification_block() {
        let v = request_body(&DeviceToken::new("tok"), &alert());
        let m = &v["message"];
        assert_eq!(m["token"], "tok");
        assert_eq!(m["android"]["priority"], "HIGH");
        assert_eq!(m["data"]["alert_id"], "a-1");
        assert_eq!(m["data"]["title"], "DB down");
        assert_eq!(m["data"]["body"], "prod-1 unreachable");
        assert!(
            m.get("notification").is_none(),
            "a notification block bypasses the alarm code"
        );
        assert!(m["android"].get("notification").is_none());
    }

    #[test]
    fn data_values_are_all_strings_as_fcm_requires() {
        let v = request_body(&DeviceToken::new("t"), &alert());
        for (k, val) in v["message"]["data"].as_object().unwrap() {
            assert!(val.is_string(), "data.{k} must be a string");
        }
    }

    #[test]
    fn data_carries_exactly_the_three_fields_the_app_reads() {
        let v = request_body(&DeviceToken::new("t"), &alert());
        let mut keys: Vec<_> = v["message"]["data"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        keys.sort();
        assert_eq!(keys, ["alert_id", "body", "title"]);
    }

    #[test]
    fn ttl_is_bounded() {
        let v = request_body(&DeviceToken::new("t"), &alert());
        assert_eq!(v["message"]["android"]["ttl"], format!("{TTL_SECONDS}s"));
    }

    #[test]
    fn json_escaping_keeps_hostile_text_inside_its_field() {
        let a = AlertMessage {
            alert_id: "x\"}".into(),
            title: "\n".into(),
            body: "\u{0}\"".into(),
        };
        let v = request_body(&DeviceToken::new("t"), &a);
        let text = serde_json::to_string(&v).unwrap();
        let back: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(back["message"]["data"]["alert_id"], "x\"}");
        assert_eq!(back["message"]["data"]["body"], "\u{0}\"");
    }

    #[test]
    fn device_token_never_prints() {
        #[derive(Debug)]
        #[allow(dead_code)]
        struct Holder<'a>(&'a DeviceToken);

        let t = DeviceToken::new("secret-token-value");
        assert!(!format!("{t:?}").contains("secret-token-value"));
        assert!(!format!("{t}").contains("secret-token-value"));
        assert!(!format!("{:?}", Holder(&t)).contains("secret-token-value"));
    }

    #[test]
    fn send_url_names_the_project() {
        assert_eq!(
            send_url("alarmant-7bbff"),
            "https://fcm.googleapis.com/v1/projects/alarmant-7bbff/messages:send"
        );
    }
}
