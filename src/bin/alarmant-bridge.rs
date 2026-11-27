// SPDX-License-Identifier: MIT OR Apache-2.0
//! `alarmant-bridge`: the Synapse human-delivery adapter (`docs/SYNAPSE-ADAPTER.md` 3.1 to 3.6,
//! CireSnave ruling 478). It runs beside `synapsed`, claims its own Synapse role (default `alarmant-bridge`, never the lane's `alarmant`) (it
//! never re-claims: a superseded or unauthorized session stops it), reads mail, raises one alert on the
//! phone server for each message that passes the gate, and when the human acknowledges, sends the
//! original sender one `[NOTICE] page-acked` mail. It has no listening socket.
//!
//! The daemon supplies only the authenticated sender; urgency and the human account are read from the
//! sender's L1 header, untrusted (`bridge::parse_envelope`). A real ack on the human's behalf does not
//! exist in Synapse, so the fallback mail of 3.2 is what goes back. No delivery, fetch or failure report
//! is ever sent to a sender (constraint 13, S2): a server that is down means the message is left unacked
//! and the daemon redelivers it.
//!
//! Environment: `SYNAPSE_HOME` (the daemon's home), `ALARMANT_BRIDGE_SERVER` and `ALARMANT_BRIDGE_KEY`,
//! `ALARMANT_BRIDGE_STATE_DIR` (open-alert file and audit file); optionally `ALARMANT_BRIDGE_ROLE` (refused if `alarmant`), `ALARMANT_BRIDGE_ACCOUNT`
//! (default `ciresnave`) and `ALARMANT_BRIDGE_SERVER_CERT_FILE`. The key is never an argument and never
//! printed.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use alarmant::bridge::{
    AuditEvent, BridgeLimits, Gate, Incoming, Refusal, Reply, audit_line, bridge_role,
    is_own_traffic, load_open, parse_envelope, reply_acked, reply_refused, save_open, utc,
};
use alarmant::bridgeclient::{AlertState, BridgeClient};
use alarmant::canary::{Canary, HeartbeatClient, canary_return, canary_text};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use serde_json::{Value, json};
use synapsed_client::{Daemon, MailError};

/// The heartbeat cadence (docs/SYNAPSE-ADAPTER.md 5.1).
const HEARTBEAT_EVERY_SECS: u64 = 60;
const TICK: Duration = Duration::from_secs(20);
/// An alert is followed for at most a day (3.6).
const FOLLOW_SECS: u64 = 86_400;

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn audit(path: &Path, line: &str) {
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{line}");
    }
}

/// Whether the error means this session is no longer ours: stop, never re-claim.
fn lost(e: &MailError) -> bool {
    matches!(e, MailError::Api { kind, .. } if kind == "superseded" || kind == "unauthorized")
}

/// The daemon connection and the role this process claims.
struct Mailbox<'a> {
    daemon: &'a Daemon,
    role: &'a str,
}

fn send(mail: &Mailbox, r: &Reply) -> Result<(), MailError> {
    let body = json!({
        "to": r.to,
        "message_id": r.message_id,
        "body_b64": B64.encode(r.text.as_bytes()),
        "mode": "L1:message-formats/1",
    });
    mail.daemon
        .call(mail.role, "/v1/send", Some(&body))
        .map(|_| ())
        .inspect_err(|e| eprintln!("alarmant-bridge: send {}: {e}", r.message_id))
}

fn env(name: &str) -> Result<String, String> {
    std::env::var(name).map_err(|_| format!("{name} is not set"))
}

fn run() -> Result<(), String> {
    let home = PathBuf::from(env("SYNAPSE_HOME")?);
    let base = env("ALARMANT_BRIDGE_SERVER")?;
    let key = env("ALARMANT_BRIDGE_KEY")?;
    let state_dir = PathBuf::from(env("ALARMANT_BRIDGE_STATE_DIR")?);
    let role = bridge_role(std::env::var("ALARMANT_BRIDGE_ROLE").ok().as_deref())?;
    let account = std::env::var("ALARMANT_BRIDGE_ACCOUNT").unwrap_or_else(|_| "ciresnave".into());
    let client = match std::env::var("ALARMANT_BRIDGE_SERVER_CERT_FILE") {
        Ok(p) if !p.trim().is_empty() => {
            let pem = std::fs::read(p.trim())
                .map_err(|_| "ALARMANT_BRIDGE_SERVER_CERT_FILE cannot be read")?;
            BridgeClient::trusting(&base, key.trim(), &pem)
        }
        _ => BridgeClient::new(&base, key.trim()),
    }
    .map_err(|e| e.to_string())?;
    // The watchdog heartbeat (docs/SYNAPSE-ADAPTER.md 5.1) is on only when its own key is set.
    let heartbeat = match std::env::var("ALARMANT_HEARTBEAT_KEY") {
        Ok(k) if !k.trim().is_empty() => Some(
            match std::env::var("ALARMANT_BRIDGE_SERVER_CERT_FILE") {
                Ok(p) if !p.trim().is_empty() => {
                    let pem = std::fs::read(p.trim())
                        .map_err(|_| "ALARMANT_BRIDGE_SERVER_CERT_FILE cannot be read")?;
                    HeartbeatClient::trusting(&base, &k, &pem)
                }
                _ => HeartbeatClient::new(&base, &k),
            }
            .map_err(|e| e.to_string())?,
        ),
        _ => None,
    };
    std::fs::create_dir_all(&state_dir).map_err(|e| e.to_string())?;
    let open_file = state_dir.join("open-alerts.jsonl");
    let audit_file = state_dir.join("audit.jsonl");
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    let daemon = Daemon::connect_existing(&home).map_err(|e| e.to_string())?;
    let mail = Mailbox {
        daemon: &daemon,
        role: &role,
    };
    let open = load_open(&open_file).map_err(|e| format!("the open-alert file: {e}"))?;
    let mut gate = Gate::with_open(&account, BridgeLimits::default(), open);
    audit(
        &audit_file,
        &audit_line(AuditEvent::Started, now(), None, None, None),
    );
    eprintln!(
        "alarmant-bridge: serving as role {role}; watchdog heartbeat {}",
        if heartbeat.is_some() { "on" } else { "off" }
    );
    let mut canary = Canary::new(now());
    let mut next_heartbeat = 0_u64;

    loop {
        let fetched = daemon.call(
            &role,
            "/v1/fetch",
            Some(&json!({"max": 10, "lease_secs": 120})),
        );
        let mut synapse_up = true;
        let messages = match fetched {
            Ok(v) => v["messages"].as_array().cloned().unwrap_or_default(),
            Err(e) if lost(&e) => {
                audit(
                    &audit_file,
                    &audit_line(AuditEvent::Superseded, now(), None, None, None),
                );
                return Err(format!("{e}"));
            }
            Err(e) => {
                eprintln!("alarmant-bridge: {e}");
                synapse_up = false;
                Vec::new()
            }
        };
        for msg in &messages {
            if let Some(token) = returned_canary(&role, msg) {
                canary.returned(&token, now());
                if let Err(e) = ack_id(&mail, msg) {
                    eprintln!("alarmant-bridge: {e}");
                }
                continue;
            }
            if let Err(e) = handle(&mail, &rt, &client, &mut gate, msg, &open_file, &audit_file) {
                if lost(&e) {
                    audit(
                        &audit_file,
                        &audit_line(AuditEvent::Superseded, now(), None, None, None),
                    );
                    return Err(format!("{e}"));
                }
                eprintln!("alarmant-bridge: {e}");
            }
        }
        if let Err(e) = poll(
            &mail,
            &rt,
            &client,
            &mut gate,
            &account,
            &open_file,
            &audit_file,
        ) {
            if lost(&e) {
                audit(
                    &audit_file,
                    &audit_line(AuditEvent::Superseded, now(), None, None, None),
                );
                return Err(format!("{e}"));
            }
            eprintln!("alarmant-bridge: {e}");
        }
        if let Some(hb) = &heartbeat {
            watch(&mail, &rt, hb, &mut canary, &mut next_heartbeat, synapse_up);
        }
        std::thread::sleep(TICK);
    }
}

/// The token when `msg` is our own canary come back (from our own role, canary subject).
fn returned_canary(role: &str, msg: &Value) -> Option<String> {
    let from = msg["from"].as_str()?;
    let text = String::from_utf8(B64.decode(msg["body_b64"].as_str()?).ok()?).ok()?;
    canary_return(from, role, &text).map(str::to_owned)
}

/// Acknowledges one fetched message by its id.
fn ack_id(mail: &Mailbox, msg: &Value) -> Result<(), MailError> {
    let id = msg["message_id"].as_str().unwrap_or_default();
    mail.daemon
        .call(mail.role, "/v1/ack", Some(&json!({"message_id": id})))
        .map(|_| ())
}

/// The watchdog's two fixed jobs (5.1): every 5 minutes send the canary (a mail to our own role that must
/// come back within 30 seconds), and every 60 seconds post one heartbeat. A failed send or post is only
/// logged: the server pages on the missing or late beat, never this process (S2, S7).
fn watch(
    mail: &Mailbox,
    rt: &tokio::runtime::Runtime,
    hb: &HeartbeatClient,
    canary: &mut Canary,
    next_heartbeat: &mut u64,
    synapse_up: bool,
) {
    let t = now();
    if canary.due(t) {
        let token = format!("c{t}");
        let reply = Reply {
            to: mail.role.to_owned(),
            message_id: format!("canary-{token}"),
            text: canary_text(&token, &utc(t)),
        };
        // Counted as in flight even when the send fails: a canary that cannot be sent is a late canary.
        canary.sent(&token, t);
        let _ = send(mail, &reply);
    }
    if t >= *next_heartbeat {
        *next_heartbeat = t + HEARTBEAT_EVERY_SECS;
        let ok = canary.ok(t);
        if let Err(e) = rt.block_on(hb.post(synapse_up, ok)) {
            eprintln!("alarmant-bridge: heartbeat not posted: {e}");
        }
    }
}

/// One message: parse, gate, raise, remember, then ack our own lease. A refusal is answered and acked;
/// a server that cannot be reached leaves the message unacked (the daemon redelivers it) and sends
/// nothing (no delivery report, S2). A crash between raising and saving can raise a message twice.
fn handle(
    mail: &Mailbox,
    rt: &tokio::runtime::Runtime,
    client: &BridgeClient,
    gate: &mut Gate,
    msg: &Value,
    open_file: &Path,
    audit_file: &Path,
) -> Result<(), MailError> {
    let id = msg["message_id"].as_str().unwrap_or_default().to_owned();
    let from = msg["from"].as_str().unwrap_or_default().to_owned();
    let text = msg["body_b64"]
        .as_str()
        .and_then(|b| B64.decode(b).ok())
        .and_then(|b| String::from_utf8(b).ok());
    let t = now();
    let ack = || {
        mail.daemon
            .call(mail.role, "/v1/ack", Some(&json!({"message_id": id})))
            .map(|_| ())
            .inspect_err(|e| eprintln!("alarmant-bridge: ack {id}: {e}"))
    };
    let refuse = |m: &Incoming, why: Refusal| -> Result<(), MailError> {
        audit(
            audit_file,
            &audit_line(AuditEvent::Refused, t, Some(m), None, Some(why)),
        );
        send(mail, &reply_refused(m, why, t))?;
        ack()
    };
    let bare = || Incoming {
        from: from.clone(),
        to: String::new(),
        urgency_high: false,
        message_id: id.clone(),
        subject: String::new(),
        body: String::new(),
    };
    if is_own_traffic(text.as_deref().unwrap_or_default()) {
        return ack();
    }
    let Some(text) = text else {
        return refuse(&bare(), Refusal::Malformed);
    };
    let m = match parse_envelope(&from, &id, &text) {
        Ok(m) => m,
        Err(why) => {
            return refuse(&bare(), why);
        }
    };
    audit(
        audit_file,
        &audit_line(AuditEvent::Received, t, Some(&m), None, None),
    );
    let alert = match gate.admit(&m, t) {
        Ok(a) => a,
        Err(why) => return refuse(&m, why),
    };
    match rt.block_on(client.raise(&alert)) {
        Ok(alert_id) => {
            gate.raised(&m, &alert_id, t);
            if let Err(e) = save_open(open_file, &gate.open()) {
                eprintln!("alarmant-bridge: cannot save the open alerts: {e}");
            }
            audit(
                audit_file,
                &audit_line(AuditEvent::Raised, t, Some(&m), Some(&alert_id), None),
            );
            ack()
        }
        Err(e) => {
            eprintln!("alarmant-bridge: not raised: {e}");
            Ok(())
        }
    }
}

/// Asks the server about each open alert; on Acknowledge, sends the one reply and forgets it. An alert
/// followed for a day, or that the server no longer knows, is dropped in silence (no expiry notice, S2).
fn poll(
    mail: &Mailbox,
    rt: &tokio::runtime::Runtime,
    client: &BridgeClient,
    gate: &mut Gate,
    account: &str,
    open_file: &Path,
    audit_file: &Path,
) -> Result<(), MailError> {
    let t = now();
    let (list, gone) = gate.to_poll(t, FOLLOW_SECS);
    let mut changed = !gone.is_empty();
    for o in list {
        match rt.block_on(client.state(&o.alert_id)) {
            Ok(AlertState::Acked) => {
                send(mail, &reply_acked(&o, account, t))?;
                gate.closed(&o.alert_id);
                audit(
                    audit_file,
                    &audit_line(AuditEvent::Acked, t, None, Some(&o.alert_id), None),
                );
                changed = true;
            }
            Ok(AlertState::Unknown) => {
                gate.closed(&o.alert_id);
                changed = true;
            }
            Ok(AlertState::Open) | Err(_) => {}
        }
    }
    if changed && let Err(e) = save_open(open_file, &gate.open()) {
        eprintln!("alarmant-bridge: cannot save the open alerts: {e}");
    }
    Ok(())
}

fn main() {
    if let Err(m) = run() {
        eprintln!("alarmant-bridge: {m}");
        std::process::exit(1);
    }
}
