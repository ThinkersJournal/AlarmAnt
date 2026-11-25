//! What the bridge says to the phone server: `POST /alert` to raise, `GET /alert/:id` to ask whether it
//! was acknowledged. The same API key as any sender, in the `Authorization` header only; the server
//! answers `{"state":"open"}` or `{"state":"acked"}` and nothing else (constraint 13).

use std::fmt;

use httpant_client::Client;

use crate::bridge::BridgeAlert;
use crate::pcclient::base_allowed;

/// Why a call failed. Local only: never sent to a sender (a delivery report is forbidden, S2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerError {
    Unreachable,
    /// The server refused the key.
    Refused,
    /// The server refused the alert (rate limit, no device, bad body): the status.
    Rejected(u16),
    InsecureBase,
    BadCertificate,
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServerError::Unreachable => f.write_str("the server could not be reached"),
            ServerError::Refused => f.write_str("the server refused the API key"),
            ServerError::Rejected(s) => write!(f, "the server answered {s}"),
            ServerError::InsecureBase => {
                f.write_str("the server address must be https (http only on loopback)")
            }
            ServerError::BadCertificate => {
                f.write_str("the server certificate file holds no usable certificate")
            }
        }
    }
}

impl std::error::Error for ServerError {}

/// Whether the human has acknowledged an alert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertState {
    Open,
    Acked,
    /// The server does not know the id (restarted without its store, or long gone).
    Unknown,
}

pub struct BridgeClient {
    base: String,
    key: String,
    client: Client,
}

impl fmt::Debug for BridgeClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BridgeClient")
            .field("base", &self.base)
            .finish_non_exhaustive()
    }
}

/// The alert id out of a `POST /alert` answer.
pub fn alert_id_from(body: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()?
        .get("alert_id")?
        .as_str()
        .map(str::to_owned)
}

/// The state out of a `GET /alert/:id` answer; anything but the two fixed shapes is not an answer.
pub fn state_from(body: &str) -> Option<AlertState> {
    match serde_json::from_str::<serde_json::Value>(body)
        .ok()?
        .get("state")?
        .as_str()?
    {
        "open" => Some(AlertState::Open),
        "acked" => Some(AlertState::Acked),
        _ => None,
    }
}

impl BridgeClient {
    pub fn new(base: &str, key: &str) -> Result<Self, ServerError> {
        if !base_allowed(base) {
            return Err(ServerError::InsecureBase);
        }
        Ok(BridgeClient {
            base: base.trim_end_matches('/').to_owned(),
            key: key.to_owned(),
            client: Client::new(),
        })
    }

    /// Also trusting the server's own self-signed certificate (a home LAN), as `PcClient::trusting`.
    pub fn trusting(base: &str, key: &str, server_cert_pem: &[u8]) -> Result<Self, ServerError> {
        let mut c = BridgeClient::new(base, key)?;
        c.client = Client::builder()
            .add_root_pem(server_cert_pem)
            .build()
            .map_err(|_| ServerError::BadCertificate)?;
        Ok(c)
    }

    /// Raises the alert; returns its id.
    pub async fn raise(&self, alert: &BridgeAlert) -> Result<String, ServerError> {
        let res = self
            .client
            .post(format!("{}/alert", self.base))
            .header("authorization", format!("Bearer {}", self.key))
            .header("content-type", "application/json")
            .body(alert.json())
            .send()
            .await
            .map_err(|_| ServerError::Unreachable)?;
        match res.status().as_u16() {
            200..=299 => {
                let text = res.text().await.map_err(|_| ServerError::Unreachable)?;
                alert_id_from(&text).ok_or(ServerError::Unreachable)
            }
            401 => Err(ServerError::Refused),
            s => Err(ServerError::Rejected(s)),
        }
    }

    /// Whether `alert_id` was acknowledged.
    pub async fn state(&self, alert_id: &str) -> Result<AlertState, ServerError> {
        let res = self
            .client
            .get(format!("{}/alert/{alert_id}", self.base))
            .header("authorization", format!("Bearer {}", self.key))
            .send()
            .await
            .map_err(|_| ServerError::Unreachable)?;
        match res.status().as_u16() {
            200 => {
                let text = res.text().await.map_err(|_| ServerError::Unreachable)?;
                state_from(&text).ok_or(ServerError::Unreachable)
            }
            401 => Err(ServerError::Refused),
            404 => Ok(AlertState::Unknown),
            _ => Err(ServerError::Unreachable),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_two_fixed_answers_read() {
        assert_eq!(state_from(r#"{"state":"open"}"#), Some(AlertState::Open));
        assert_eq!(state_from(r#"{"state":"acked"}"#), Some(AlertState::Acked));
        assert_eq!(state_from(r#"{"state":"second"}"#), None);
        assert_eq!(state_from("nope"), None);
        assert_eq!(alert_id_from(r#"{"alert_id":"a1"}"#).as_deref(), Some("a1"));
        assert_eq!(alert_id_from("{}"), None);
    }

    #[test]
    fn a_key_is_not_sent_over_plain_http_off_this_machine() {
        assert_eq!(
            BridgeClient::new("http://example.com", "k").unwrap_err(),
            ServerError::InsecureBase
        );
        assert!(BridgeClient::new("https://example.com", "k").is_ok());
        assert!(BridgeClient::new("http://127.0.0.1:8080", "k").is_ok());
    }
}
