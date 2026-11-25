//! The bridge's side of the watchdog (docs/SYNAPSE-ADAPTER.md 5.1): the 5-minute canary and the
//! 60-second heartbeat it feeds. Pure logic plus one small client; the bridge binary owns the loop
//! and the Synapse calls (a message from `alarmant` to itself must come back within 30 seconds).
//!
//! Fixed numbers, no learning, no score (S7). The heartbeat carries no secret and no text: only
//! `{"synapse":"up|down","canary":"ok|late","bridge":"up"}`.

use std::fmt;

use httpant_client::Client;

use crate::pcclient::base_allowed;

/// Seconds between canary messages.
pub const CANARY_EVERY_SECS: u64 = 300;
/// How long a canary may take to come back.
pub const CANARY_WAIT_SECS: u64 = 30;
/// Consecutive failed canaries before the heartbeat says `late` (a single late canary pages nothing;
/// the server then needs 3 such heartbeats in a row, docs 5.2).
pub const FAILS_FOR_LATE: u32 = 2;

/// The canary's bookkeeping, on a clock the caller passes in.
#[derive(Debug, Clone)]
pub struct Canary {
    next_at: u64,
    /// The probe in flight: its token and the time it is overdue.
    outstanding: Option<(String, u64)>,
    fails: u32,
}

impl Canary {
    /// The first canary is due at `now`.
    pub fn new(now: u64) -> Self {
        Canary {
            next_at: now,
            outstanding: None,
            fails: 0,
        }
    }

    /// Whether to send a canary now (none in flight and the time has come).
    pub fn due(&mut self, now: u64) -> bool {
        self.settle(now);
        self.outstanding.is_none() && now >= self.next_at
    }

    /// A canary carrying `token` was sent at `now`.
    pub fn sent(&mut self, token: &str, now: u64) {
        self.settle(now);
        self.outstanding = Some((token.to_owned(), now + CANARY_WAIT_SECS));
        self.next_at = now + CANARY_EVERY_SECS;
    }

    /// A message with `token` came back at `now`. Only the token in flight counts, and only in time.
    pub fn returned(&mut self, token: &str, now: u64) {
        if let Some((t, deadline)) = &self.outstanding
            && t == token
        {
            if now <= *deadline {
                self.fails = 0;
            } else {
                self.fails += 1;
            }
            self.outstanding = None;
        }
    }

    fn settle(&mut self, now: u64) {
        if let Some((_, deadline)) = &self.outstanding
            && now > *deadline
        {
            self.fails += 1;
            self.outstanding = None;
        }
    }

    /// What the heartbeat says: `true` is `ok`.
    pub fn ok(&mut self, now: u64) -> bool {
        self.settle(now);
        self.fails < FAILS_FOR_LATE
    }
}

/// The canary mail the bridge sends to its own role: a valid L1 `[NOTICE]`, carrying only a token and the
/// time. The bridge acknowledges and drops it when it comes back; it never reaches the gate or the phone.
pub fn canary_text(token: &str, when: &str) -> String {
    format!(
        "[NOTICE] page-canary affects=alarmant-bridge when={when} action=none
note: id={token}"
    )
}

/// The token of a canary that came back: the mail must be from our own role (`role` or `role@account`) and
/// have the canary subject and a plain `note: id=` line (letters, digits, `-`, at most 64). Anything else is
/// ordinary mail.
pub fn canary_return<'a>(from: &str, role: &str, text: &'a str) -> Option<&'a str> {
    if from.split('@').next() != Some(role) || !text.starts_with("[NOTICE] page-canary ") {
        return None;
    }
    let token = text.lines().find_map(|l| l.strip_prefix("note: id="))?;
    let plain = !token.is_empty()
        && token.len() <= 64
        && token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-');
    plain.then_some(token)
}

/// The fixed-shape heartbeat body.
pub fn heartbeat_body(synapse_up: bool, canary_ok: bool) -> String {
    format!(
        r#"{{"synapse":"{}","canary":"{}","bridge":"up"}}"#,
        if synapse_up { "up" } else { "down" },
        if canary_ok { "ok" } else { "late" }
    )
}

/// Why a heartbeat was not accepted. Local only; never carries the key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeartbeatError {
    /// Not an https base (or http on this machine).
    InsecureBase,
    /// The server could not be reached.
    Unreachable,
    /// 401: the key was refused.
    Refused,
    /// Any other answer, such as the 429 cap.
    Rejected,
}

impl fmt::Display for HeartbeatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            HeartbeatError::InsecureBase => {
                "the server address must be https (or http on this machine)"
            }
            HeartbeatError::Unreachable => "the server could not be reached",
            HeartbeatError::Refused => "the heartbeat key was refused",
            HeartbeatError::Rejected => "the server did not accept the heartbeat",
        })
    }
}

impl std::error::Error for HeartbeatError {}

/// Posts heartbeats with the heartbeat key, which can post heartbeats and nothing else.
pub struct HeartbeatClient {
    base: String,
    key: String,
    client: Client,
}

impl fmt::Debug for HeartbeatClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HeartbeatClient")
            .field("base", &self.base)
            .finish_non_exhaustive()
    }
}

impl HeartbeatClient {
    pub fn new(base: &str, key: &str) -> Result<Self, HeartbeatError> {
        if !base_allowed(base) {
            return Err(HeartbeatError::InsecureBase);
        }
        Ok(HeartbeatClient {
            base: base.trim_end_matches('/').to_owned(),
            key: key.trim().to_owned(),
            client: Client::new(),
        })
    }

    /// Like [`HeartbeatClient::new`], trusting the server's own self-signed certificate.
    pub fn trusting(base: &str, key: &str, server_cert_pem: &[u8]) -> Result<Self, HeartbeatError> {
        let mut c = HeartbeatClient::new(base, key)?;
        c.client = Client::builder()
            .add_root_pem(server_cert_pem)
            .build()
            .map_err(|_| HeartbeatError::InsecureBase)?;
        Ok(c)
    }

    /// One heartbeat.
    pub async fn post(&self, synapse_up: bool, canary_ok: bool) -> Result<(), HeartbeatError> {
        let res = self
            .client
            .post(format!("{}/heartbeat", self.base))
            .header("authorization", format!("Bearer {}", self.key))
            .body(heartbeat_body(synapse_up, canary_ok))
            .send()
            .await
            .map_err(|_| HeartbeatError::Unreachable)?;
        match res.status().as_u16() {
            200 => Ok(()),
            401 => Err(HeartbeatError::Refused),
            _ => Err(HeartbeatError::Rejected),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ack::AckToken;
    use crate::heartbeat::{HeartbeatConfig, heartbeat_route};
    use crate::watchdog::{Component, Watchdog, parse_heartbeat};
    use httpant_server::Server;
    use std::sync::{Arc, Mutex};

    #[test]
    fn a_canary_text_round_trips_and_is_a_valid_l1_notice() {
        let text = canary_text("c1795533937", "2026-11-25T06:00:00Z");
        assert!(text.starts_with("[NOTICE] page-canary affects="), "{text}");
        assert!(text.contains(" when=2026-11-25T06:00:00Z action=none"));
        assert_eq!(
            canary_return("alarmant-bridge@ciresnave", "alarmant-bridge", &text),
            Some("c1795533937")
        );
        // the bare role also counts as ourselves
        assert_eq!(
            canary_return("alarmant-bridge", "alarmant-bridge", &text),
            Some("c1795533937")
        );
    }

    #[test]
    fn only_our_own_canary_counts_as_one() {
        let text = canary_text("c1", "2026-11-25T06:00:00Z");
        assert_eq!(
            canary_return("someone@ciresnave", "alarmant-bridge", &text),
            None
        );
        assert_eq!(
            canary_return("alarmant-bridge2@ciresnave", "alarmant-bridge", &text),
            None
        );
        let other = "[NOTICE] topic to=ciresnave urgency=high
note: id=c1";
        assert_eq!(
            canary_return("alarmant-bridge@ciresnave", "alarmant-bridge", other),
            None
        );
        let no_id = "[NOTICE] page-canary affects=x when=y action=none";
        assert_eq!(
            canary_return("alarmant-bridge@ciresnave", "alarmant-bridge", no_id),
            None
        );
        let bad = "[NOTICE] page-canary affects=x when=y action=none
note: id=a b";
        assert_eq!(
            canary_return("alarmant-bridge@ciresnave", "alarmant-bridge", bad),
            None
        );
    }

    #[test]
    fn the_body_is_the_fixed_shape_the_server_accepts() {
        for (up, ok) in [(true, true), (true, false), (false, true), (false, false)] {
            let b = heartbeat_body(up, ok);
            let hb = parse_heartbeat(&b).expect(&b);
            assert_eq!((hb.synapse_up, hb.canary_ok), (up, ok));
        }
        assert_eq!(
            heartbeat_body(true, true),
            r#"{"synapse":"up","canary":"ok","bridge":"up"}"#
        );
    }

    #[test]
    fn the_first_canary_is_due_at_once_then_every_five_minutes() {
        let mut c = Canary::new(1000);
        assert!(c.due(1000));
        c.sent("t1", 1000);
        assert!(!c.due(1001), "one in flight");
        c.returned("t1", 1005);
        assert!(!c.due(1000 + 299));
        assert!(c.due(1000 + 300));
    }

    #[test]
    fn a_canary_that_returns_in_time_keeps_the_heartbeat_ok() {
        let mut c = Canary::new(0);
        c.sent("t1", 0);
        c.returned("t1", CANARY_WAIT_SECS);
        assert!(c.ok(CANARY_WAIT_SECS + 1));
    }

    #[test]
    fn one_missed_canary_is_not_late_but_two_in_a_row_are() {
        let mut c = Canary::new(0);
        c.sent("t1", 0);
        assert!(c.ok(CANARY_WAIT_SECS + 1), "one miss: still ok");
        assert!(c.due(CANARY_EVERY_SECS));
        c.sent("t2", CANARY_EVERY_SECS);
        assert!(
            !c.ok(CANARY_EVERY_SECS + CANARY_WAIT_SECS + 1),
            "two misses: late"
        );
        // recovery: one in-time return clears it
        c.sent("t3", 2 * CANARY_EVERY_SECS);
        c.returned("t3", 2 * CANARY_EVERY_SECS + 2);
        assert!(c.ok(2 * CANARY_EVERY_SECS + 3));
    }

    #[test]
    fn a_late_return_and_a_stray_token_do_not_count_as_good() {
        let mut c = Canary::new(0);
        c.sent("t1", 0);
        c.returned("other", 5);
        assert!(c.ok(5), "a stray token changes nothing yet");
        c.returned("t1", CANARY_WAIT_SECS + 5);
        c.sent("t2", CANARY_EVERY_SECS);
        c.returned("t2", CANARY_EVERY_SECS + CANARY_WAIT_SECS + 5);
        assert!(
            !c.ok(CANARY_EVERY_SECS + CANARY_WAIT_SECS + 6),
            "two late returns"
        );
    }

    #[test]
    fn an_insecure_base_is_refused() {
        assert_eq!(
            HeartbeatClient::new("http://example.com", "k").unwrap_err(),
            HeartbeatError::InsecureBase
        );
    }

    #[tokio::test]
    async fn the_client_and_the_route_agree_end_to_end() {
        // A fixed, obviously fake test value, built at run time so no secret-looking literal is committed.
        let key = format!("EXAMPLE-NOT-A-SECRET-{}", "0".repeat(24));
        let key = key.as_str();
        let wd = Arc::new(Mutex::new(Watchdog::new(1_000)));
        let srv = heartbeat_route(
            Server::new(),
            HeartbeatConfig {
                key: AckToken::new(key),
                watchdog: Arc::clone(&wd),
                sink: Arc::new(|_| {}),
                clock: Arc::new(|| 1_010),
            },
        )
        .bind("127.0.0.1:0")
        .await
        .unwrap();
        let base = format!("http://{}", srv.addr());
        let good = HeartbeatClient::new(&base, key).unwrap();
        assert_eq!(good.post(true, true).await, Ok(()));
        wd.lock().unwrap().tick(1_010 + 179);
        assert!(
            !wd.lock().unwrap().is_open(Component::Pc),
            "the beat counted"
        );
        let bad = HeartbeatClient::new(&base, "wrong-key").unwrap();
        assert_eq!(bad.post(true, true).await, Err(HeartbeatError::Refused));
        srv.shutdown().await.unwrap();
    }
}
