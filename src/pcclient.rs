//! The PC alarm program's logic, free of the operating system: ask the server, start and stop a
//! siren, acknowledge on the operator's say-so.
//!
//! Constraints 17 to 22 (`PATENT-PC-ALARM-READING.md`): the program only **asks** (`GET /pc/alarm`)
//! and sends one kind of message, an Acknowledge when the operator stops the alarm on purpose
//! (`POST /pc/ack/:id`). It sends no status of any kind: not that it polled, sounded, failed to
//! sound or was dismissed. The server's answer is only a list of alert ids; volume, unmute and the
//! siren are this program's own. A server that cannot be reached changes nothing here: a siren that
//! is sounding keeps sounding.

use std::fmt;

use httpant_client::Client;

/// What the program can do to the sound. The Windows implementation is in `alarmant-pc`.
pub trait Siren {
    /// Unmute, raise the volume to maximum, start the siren. Must be harmless to call twice.
    fn start(&mut self);
    /// Called on every poll while it is sounding: put the volume and unmute back if someone
    /// changed them. Does not restart the siren.
    fn keep(&mut self);
    /// Stop the siren.
    fn stop(&mut self);
}

/// Why a request failed. Local only: never sent anywhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcError {
    /// The server could not be reached or answered something unreadable.
    Unreachable,
    /// The server refused the PC key.
    Refused,
    /// The base address is not https (or http on loopback).
    InsecureBase,
    /// The server certificate file holds no usable certificate.
    BadCertificate,
}

impl fmt::Display for PcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            PcError::Unreachable => "the server could not be reached",
            PcError::Refused => "the server refused the PC key",
            PcError::InsecureBase => "the server address must be https (http only on loopback)",
            PcError::BadCertificate => "the server certificate file holds no usable certificate",
        })
    }
}

impl std::error::Error for PcError {}

/// Whether a key may be sent to `base`: https anywhere, plain http only to this machine.
pub fn base_allowed(base: &str) -> bool {
    if base.starts_with("https://") {
        return true;
    }
    let Some(rest) = base.strip_prefix("http://") else {
        return false;
    };
    let host = rest.split('/').next().unwrap_or("");
    let host = if let Some(h) = host.strip_prefix('[') {
        h.split(']').next().unwrap_or("")
    } else {
        host.rsplit_once(':').map_or(host, |(h, _)| h)
    };
    host == "localhost"
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
}

/// The server and the PC key. The key goes in the `Authorization` header only.
pub struct PcClient {
    base: String,
    key: String,
    client: Client,
}

impl fmt::Debug for PcClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PcClient")
            .field("base", &self.base)
            .finish_non_exhaustive()
    }
}

impl PcClient {
    pub fn new(base: &str, key: &str) -> Result<Self, PcError> {
        if !base_allowed(base) {
            return Err(PcError::InsecureBase);
        }
        Ok(PcClient {
            base: base.trim_end_matches('/').to_owned(),
            key: key.to_owned(),
            client: Client::new(),
        })
    }

    /// Like [`PcClient::new`], also trusting the certificate(s) in `server_cert_pem`: for a server that
    /// serves a self-signed certificate (a home LAN, `ALARMANT_TLS_SELF_SIGNED_NAMES`). The PEM is the
    /// server's own `tls-cert.pem`, copied to this PC over a channel the operator trusts; it adds to the
    /// default roots and replaces nothing. A PEM with no certificate is refused.
    pub fn trusting(base: &str, key: &str, server_cert_pem: &[u8]) -> Result<Self, PcError> {
        let mut c = PcClient::new(base, key)?;
        c.client = Client::builder()
            .add_root_pem(server_cert_pem)
            .build()
            .map_err(|_| PcError::BadCertificate)?;
        Ok(c)
    }

    /// Alert ids whose PC alarm should be sounding now.
    pub async fn active(&self) -> Result<Vec<String>, PcError> {
        let res = self
            .client
            .get(format!("{}/pc/alarm", self.base))
            .header("authorization", format!("Bearer {}", self.key))
            .send()
            .await
            .map_err(|_| PcError::Unreachable)?;
        match res.status().as_u16() {
            200 => {
                let text = res.text().await.map_err(|_| PcError::Unreachable)?;
                let v: serde_json::Value =
                    serde_json::from_str(&text).map_err(|_| PcError::Unreachable)?;
                Ok(v.get("alert_ids")
                    .and_then(|a| a.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|s| s.as_str().map(str::to_owned))
                            .collect()
                    })
                    .ok_or(PcError::Unreachable)?)
            }
            401 => Err(PcError::Refused),
            _ => Err(PcError::Unreachable),
        }
    }

    /// Observer B's check: `GET /health` answers 200 with the fixed text. Sends no key and no status;
    /// the answer never leaves this process.
    pub async fn healthy(&self) -> bool {
        let Ok(res) = self
            .client
            .get(format!("{}/health", self.base))
            .send()
            .await
        else {
            return false;
        };
        res.status().as_u16() == 200
            && res
                .text()
                .await
                .is_ok_and(|t| t.trim() == crate::hostwatch::HEALTH_TEXT)
    }

    /// The operator's Acknowledge for one alert.
    pub async fn acknowledge(&self, alert_id: &str) -> Result<(), PcError> {
        let res = self
            .client
            .post(format!("{}/pc/ack/{alert_id}", self.base))
            .header("authorization", format!("Bearer {}", self.key))
            .send()
            .await
            .map_err(|_| PcError::Unreachable)?;
        match res.status().as_u16() {
            200 => Ok(()),
            401 => Err(PcError::Refused),
            // 404: already acknowledged elsewhere, which is what the operator wanted.
            404 => Ok(()),
            _ => Err(PcError::Unreachable),
        }
    }
}

/// Keeps the siren in step with the server's list.
pub struct Watcher<S: Siren> {
    siren: S,
    sounding: bool,
}

impl<S: Siren> Watcher<S> {
    pub fn new(siren: S) -> Self {
        Watcher {
            siren,
            sounding: false,
        }
    }

    pub fn is_sounding(&self) -> bool {
        self.sounding
    }

    /// Applies one poll result. A failed poll changes nothing.
    pub fn observe(&mut self, polled: &Result<Vec<String>, PcError>) {
        let Ok(ids) = polled else { return };
        match (ids.is_empty(), self.sounding) {
            (false, false) => {
                self.siren.start();
                self.sounding = true;
            }
            (true, true) => {
                self.siren.stop();
                self.sounding = false;
            }
            (false, true) => self.siren.keep(),
            (true, false) => {}
        }
    }

    /// The operator chose to stop: silence now. The caller then acknowledges each alert.
    pub fn operator_stop(&mut self) {
        if self.sounding {
            self.siren.stop();
            self.sounding = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct Log(Arc<Mutex<Vec<&'static str>>>);
    impl Siren for Log {
        fn start(&mut self) {
            self.0.lock().unwrap().push("start");
        }
        fn keep(&mut self) {
            self.0.lock().unwrap().push("keep");
        }
        fn stop(&mut self) {
            self.0.lock().unwrap().push("stop");
        }
    }

    /// A real TLS handshake: a server with a self-signed certificate is reached only by a client that was
    /// given that certificate, and a different certificate is refused.
    #[tokio::test]
    async fn a_self_signed_server_is_trusted_only_with_its_own_certificate() {
        use crate::ack::MemoryStore;
        use crate::channel::{PcAlarmBoard, PcAlarmConfig, pc_alarm_route};
        let made = rcgen::generate_simple_self_signed(vec!["127.0.0.1".to_owned()]).unwrap();
        let (cert, key) = (made.cert.pem(), made.signing_key.serialize_pem());
        let acceptor = crate::tls::acceptor(cert.as_bytes(), key.as_bytes()).unwrap();
        let server = pc_alarm_route(
            httpant_server::Server::new(),
            PcAlarmConfig {
                pc_key: crate::ack::AckToken::new("pc-key-0123456789abcdef0123456789abcdef"),
                board: Arc::new(PcAlarmBoard::default()),
                alerts: Arc::new(MemoryStore::default()),
            },
        )
        .tls(acceptor)
        .bind("127.0.0.1:0")
        .await
        .unwrap();
        let base = format!("https://{}", server.addr());
        let key = "pc-key-0123456789abcdef0123456789abcdef";
        let trusting = PcClient::trusting(&base, key, cert.as_bytes()).unwrap();
        assert_eq!(trusting.active().await, Ok(vec![]));
        // control: the same server, a client that was not given the certificate
        assert_eq!(
            PcClient::new(&base, key).unwrap().active().await,
            Err(PcError::Unreachable)
        );
        // another certificate is not this server's
        let other = rcgen::generate_simple_self_signed(vec!["127.0.0.1".to_owned()])
            .unwrap()
            .cert
            .pem();
        let wrong = PcClient::trusting(&base, key, other.as_bytes()).unwrap();
        assert_eq!(wrong.active().await, Err(PcError::Unreachable));
        assert_eq!(
            PcClient::trusting(&base, key, b"not pem").err(),
            Some(PcError::BadCertificate)
        );
    }

    fn ids(v: &[&str]) -> Result<Vec<String>, PcError> {
        Ok(v.iter().map(|s| (*s).to_owned()).collect())
    }

    #[test]
    fn sounds_while_the_list_is_non_empty_and_once() {
        let log = Log::default();
        let mut w = Watcher::new(log.clone());
        w.observe(&ids(&[]));
        w.observe(&ids(&["a"]));
        w.observe(&ids(&["a"]));
        assert!(w.is_sounding());
        w.observe(&ids(&[]));
        assert!(!w.is_sounding());
        assert_eq!(*log.0.lock().unwrap(), vec!["start", "keep", "stop"]);
    }

    #[test]
    fn an_unreachable_server_changes_nothing_either_way() {
        let log = Log::default();
        let mut w = Watcher::new(log.clone());
        w.observe(&Err(PcError::Unreachable));
        assert!(!w.is_sounding(), "never starts on an error");
        w.observe(&ids(&["a"]));
        w.observe(&Err(PcError::Unreachable));
        w.observe(&Err(PcError::Refused));
        assert!(w.is_sounding(), "keeps sounding through an outage");
    }

    #[test]
    fn operator_stop_silences_once() {
        let log = Log::default();
        let mut w = Watcher::new(log.clone());
        w.operator_stop();
        w.observe(&ids(&["a"]));
        w.operator_stop();
        w.operator_stop();
        assert_eq!(*log.0.lock().unwrap(), vec!["start", "stop"]);
    }

    #[test]
    fn the_key_only_goes_to_https_or_this_machine() {
        assert!(PcClient::new("https://alarm.example.com", "k").is_ok());
        assert!(PcClient::new("http://127.0.0.1:8080", "k").is_ok());
        assert!(PcClient::new("http://[::1]:8080", "k").is_ok());
        assert!(PcClient::new("http://alarm.example.com", "k").is_err());
        assert!(PcClient::new("http://127.0.0.1.evil.example", "k").is_err());
        assert!(PcClient::new("ftp://x", "k").is_err());
    }

    #[test]
    fn debug_never_shows_the_key() {
        let c = PcClient::new("https://a.example", "sekret-key").unwrap();
        assert!(!format!("{c:?}").contains("sekret"));
    }

    // End to end against the real routes.
    #[tokio::test]
    async fn polls_and_acknowledges_against_the_server() {
        use crate::ack::AlertStore;
        use crate::ack::{AckToken, AlertRecord, MemoryStore};
        use crate::channel::{PcAlarmBoard, PcAlarmConfig, pc_alarm_route};
        let store = Arc::new(MemoryStore::default());
        store.insert(AlertRecord::sent("a-1", 1, AckToken::new("t")));
        let board = Arc::new(PcAlarmBoard::default());
        board.raise("a-1");
        let srv = pc_alarm_route(
            httpant_server::Server::new(),
            PcAlarmConfig {
                pc_key: AckToken::new("pc-key-0123456789"),
                board,
                alerts: store,
            },
        )
        .bind("127.0.0.1:0")
        .await
        .unwrap();
        let base = format!("http://{}", srv.addr());
        let good = PcClient::new(&base, "pc-key-0123456789").unwrap();
        assert_eq!(good.active().await.unwrap(), vec!["a-1".to_owned()]);
        let bad = PcClient::new(&base, "wrong").unwrap();
        assert_eq!(bad.active().await, Err(PcError::Refused));
    }
}
