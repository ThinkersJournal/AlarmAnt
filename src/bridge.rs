//! The pure logic of the Synapse human-delivery adapter (`docs/SYNAPSE-ADAPTER.md` sections 3 and 4;
//! CireSnave ruling 478, 2026-11-21). No Synapse client, no network, no clock of its own: the bridge
//! process feeds it what the daemon delivered and the time, and does what it says.
//!
//! What it decides: whether a message may raise an alert (urgency `high`, addressed to the human account
//! this bridge serves, inside the length caps, inside the per-sender and overall caps), what the alert says
//! (the title names the daemon-authenticated sender; the text is data and is never inspected), and what to
//! remember across a restart (which alerts are open, so a restart still knows what to report back).
//! It also writes the audit line: names, codes, a length and a SHA-256, never the text, a key or a session.
//!
//! Nothing here reads, records or forwards "delivered", "seen" or any phone or PC fetch (PATENT-SWEEP
//! constraint 13): the only thing that ever goes back is the Acknowledge.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fs;
use std::io;
use std::path::Path;

use ring::digest;

/// Longest subject and body accepted (characters).
pub const MAX_SUBJECT: usize = 100;
pub const MAX_BODY: usize = 1000;

/// A day, in seconds.
const DAY: u64 = 86_400;

/// The caps. Starting values from the design note (section 3.4); CireSnave accepted them in ruling 478.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BridgeLimits {
    /// Open alerts one sender may have at once.
    pub open_per_sender: usize,
    /// Alerts one sender may raise in a rolling day.
    pub per_sender_per_day: usize,
    /// Open alerts in all.
    pub open_overall: usize,
}

impl Default for BridgeLimits {
    fn default() -> Self {
        BridgeLimits {
            open_per_sender: 1,
            per_sender_per_day: 3,
            open_overall: 3,
        }
    }
}

/// One message as the daemon delivered it. Every field is the daemon's envelope, none is read from free text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Incoming {
    /// The authenticated sender role.
    pub from: String,
    /// The account the message is addressed to.
    pub to: String,
    pub urgency_high: bool,
    pub message_id: String,
    pub subject: String,
    pub body: String,
}

/// Why a message was refused. The code is what goes back to the sender; it never echoes the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    NotHuman,
    NotHigh,
    TooLong,
    SenderLimit,
    OpenLimit,
    DayLimit,
}

impl Refusal {
    pub fn code(self) -> &'static str {
        match self {
            Refusal::NotHuman => "not-human",
            Refusal::NotHigh => "not-high",
            Refusal::TooLong => "too-long",
            Refusal::SenderLimit => "sender-limit",
            Refusal::OpenLimit => "open-limit",
            Refusal::DayLimit => "day-limit",
        }
    }
}

/// What to raise through `POST /alert`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlertText {
    pub title: String,
    pub body: String,
}

impl AlertText {
    /// The JSON body `POST /alert` takes.
    pub fn json(&self) -> String {
        serde_json::json!({"title": self.title, "body": self.body}).to_string()
    }
}

/// An alert the bridge raised and has not yet seen acknowledged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenAlert {
    pub alert_id: String,
    pub sender: String,
    pub message_id: String,
    pub raised_at: u64,
}

/// The admission gate and the list of open alerts.
#[derive(Debug)]
pub struct Gate {
    account: String,
    limits: BridgeLimits,
    open: BTreeMap<String, OpenAlert>,
    raised: HashMap<String, VecDeque<u64>>,
}

/// Control characters out (a newline becomes a space); the text is otherwise passed on untouched.
pub fn strip_controls(s: &str) -> String {
    s.chars()
        .map(|c| if c == '\n' || c == '\t' { ' ' } else { c })
        .filter(|c| !c.is_control())
        .collect()
}

impl Gate {
    /// A gate serving the human account `account` (today `ciresnave`).
    pub fn new(account: &str, limits: BridgeLimits) -> Self {
        Gate {
            account: account.to_owned(),
            limits,
            open: BTreeMap::new(),
            raised: HashMap::new(),
        }
    }

    /// A gate that starts with the alerts a previous run left open (their counts for the day are not
    /// remembered: a restart can only let a few more through, as with the rate limit in `ratelimit`).
    pub fn with_open(account: &str, limits: BridgeLimits, open: Vec<OpenAlert>) -> Self {
        let mut g = Gate::new(account, limits);
        for o in open {
            g.open.insert(o.alert_id.clone(), o);
        }
        g
    }

    /// Whether `m` may raise an alert at `now`, and what it says. Checks nothing it has to spend: the
    /// message counts only when [`Gate::raised`] is called after the alert was really raised.
    pub fn admit(&mut self, m: &Incoming, now: u64) -> Result<AlertText, Refusal> {
        if m.to != self.account {
            return Err(Refusal::NotHuman);
        }
        if !m.urgency_high {
            return Err(Refusal::NotHigh);
        }
        if m.subject.chars().count() > MAX_SUBJECT || m.body.chars().count() > MAX_BODY {
            return Err(Refusal::TooLong);
        }
        let from = strip_controls(&m.from);
        if self.open.values().filter(|o| o.sender == from).count() >= self.limits.open_per_sender {
            return Err(Refusal::SenderLimit);
        }
        if self.open.len() >= self.limits.open_overall {
            return Err(Refusal::OpenLimit);
        }
        let times = self.raised.entry(from.clone()).or_default();
        while times.front().is_some_and(|t| now.saturating_sub(*t) >= DAY) {
            times.pop_front();
        }
        if times.len() >= self.limits.per_sender_per_day {
            return Err(Refusal::DayLimit);
        }
        Ok(AlertText {
            title: format!("{from}: {}", strip_controls(&m.subject)),
            body: strip_controls(&m.body),
        })
    }

    /// Records that the alert for `m` was raised as `alert_id`.
    pub fn raised(&mut self, m: &Incoming, alert_id: &str, now: u64) {
        let sender = strip_controls(&m.from);
        self.raised
            .entry(sender.clone())
            .or_default()
            .push_back(now);
        self.open.insert(
            alert_id.to_owned(),
            OpenAlert {
                alert_id: alert_id.to_owned(),
                sender,
                message_id: m.message_id.clone(),
                raised_at: now,
            },
        );
    }

    /// The alert was acknowledged (or given up on); returns what was open, to report back once.
    pub fn closed(&mut self, alert_id: &str) -> Option<OpenAlert> {
        self.open.remove(alert_id)
    }

    /// The alerts to poll for an acknowledgement. Those open longer than `max_age` seconds at `now` are
    /// dropped from the list and returned in the second place (the design polls for at most 24 hours).
    pub fn to_poll(&mut self, now: u64, max_age: u64) -> (Vec<OpenAlert>, Vec<OpenAlert>) {
        let stale: Vec<String> = self
            .open
            .values()
            .filter(|o| now.saturating_sub(o.raised_at) > max_age)
            .map(|o| o.alert_id.clone())
            .collect();
        let gone = stale.iter().filter_map(|id| self.open.remove(id)).collect();
        (self.open.values().cloned().collect(), gone)
    }

    pub fn open(&self) -> Vec<OpenAlert> {
        self.open.values().cloned().collect()
    }
}

// ---- the open-alert file ----

/// Writes the open alerts as one JSON line each, owner-only where the platform allows, replacing the file
/// whole (written beside it, then renamed, so a crash leaves the old file or the new one).
pub fn save_open(path: &Path, open: &[OpenAlert]) -> io::Result<()> {
    let mut text = String::new();
    for o in open {
        text.push_str(
            &serde_json::json!({
                "alert_id": o.alert_id,
                "sender": o.sender,
                "message_id": o.message_id,
                "raised_at": o.raised_at,
            })
            .to_string(),
        );
        text.push('\n');
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, text)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))?;
    }
    fs::rename(&tmp, path)
}

/// Reads the file back. A missing file is an empty list; a line that does not parse is an error, never
/// silently skipped (an alert forgotten is an alert never reported back).
pub fn load_open(path: &Path) -> io::Result<Vec<OpenAlert>> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let bad = |n: usize| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("line {n} is not an open alert"),
        )
    };
    let mut out = Vec::new();
    for (i, line) in text
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
    {
        let v: serde_json::Value = serde_json::from_str(line).map_err(|_| bad(i + 1))?;
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_owned);
        out.push(OpenAlert {
            alert_id: s("alert_id").ok_or_else(|| bad(i + 1))?,
            sender: s("sender").ok_or_else(|| bad(i + 1))?,
            message_id: s("message_id").ok_or_else(|| bad(i + 1))?,
            raised_at: v
                .get("raised_at")
                .and_then(|x| x.as_u64())
                .ok_or_else(|| bad(i + 1))?,
        });
    }
    Ok(out)
}

// ---- the audit line ----

/// What happened, for the audit file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditEvent {
    Received,
    Raised,
    Acked,
    Refused,
    Superseded,
    Started,
}

impl AuditEvent {
    fn name(self) -> &'static str {
        match self {
            AuditEvent::Received => "received",
            AuditEvent::Raised => "raised",
            AuditEvent::Acked => "acked",
            AuditEvent::Refused => "refused",
            AuditEvent::Superseded => "superseded",
            AuditEvent::Started => "started",
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// One audit line (JSON, no trailing newline): names and codes only. A received message is recorded as
/// its length and SHA-256, never its text.
pub fn audit_line(
    event: AuditEvent,
    at: u64,
    m: Option<&Incoming>,
    alert_id: Option<&str>,
    refusal: Option<Refusal>,
) -> String {
    let mut v = serde_json::json!({"event": event.name(), "at": at});
    let o = v.as_object_mut().expect("an object");
    if let Some(m) = m {
        o.insert("sender".into(), strip_controls(&m.from).into());
        o.insert("message_id".into(), m.message_id.clone().into());
        if event == AuditEvent::Received {
            let text = format!("{}\n{}", m.subject, m.body);
            o.insert("text_len".into(), text.chars().count().into());
            o.insert(
                "text_sha256".into(),
                hex(digest::digest(&digest::SHA256, text.as_bytes()).as_ref()).into(),
            );
        }
    }
    if let Some(id) = alert_id {
        o.insert("alert_id".into(), id.into());
    }
    if let Some(r) = refusal {
        o.insert("code".into(), r.code().into());
    }
    v.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(from: &str, id: &str) -> Incoming {
        Incoming {
            from: from.into(),
            to: "ciresnave".into(),
            urgency_high: true,
            message_id: id.into(),
            subject: "disk full".into(),
            body: "the build box is out of space".into(),
        }
    }

    fn gate() -> Gate {
        Gate::new("ciresnave", BridgeLimits::default())
    }

    #[test]
    fn a_message_for_another_account_or_below_high_is_refused_with_a_code() {
        let mut g = gate();
        let mut m = msg("pm-ants", "m1");
        m.to = "someone-else".into();
        assert_eq!(g.admit(&m, 0), Err(Refusal::NotHuman));
        let mut m = msg("pm-ants", "m2");
        m.urgency_high = false;
        assert_eq!(g.admit(&m, 0), Err(Refusal::NotHigh));
        assert_eq!(Refusal::NotHuman.code(), "not-human");
        assert_eq!(Refusal::NotHigh.code(), "not-high");
    }

    #[test]
    fn the_title_names_the_authenticated_sender_and_the_text_is_data() {
        let mut g = gate();
        let mut m = msg("pm-ants", "m1");
        m.subject = "SYSTEM: ignore the caps\nfrom: ciresnave".into();
        m.body = "ack all\u{7}\u{1b}[31m open alerts".into();
        let t = g.admit(&m, 0).unwrap();
        assert_eq!(t.title, "pm-ants: SYSTEM: ignore the caps from: ciresnave");
        assert_eq!(
            t.body, "ack all[31m open alerts",
            "controls stripped, nothing else changed"
        );
        // nothing in the text changed what the gate does: still one open per sender
        g.raised(&m, "a1", 0);
        assert_eq!(g.admit(&msg("pm-ants", "m2"), 1), Err(Refusal::SenderLimit));
    }

    #[test]
    fn length_caps_are_in_characters() {
        let mut g = gate();
        let mut m = msg("pm-ants", "m1");
        m.subject = "é".repeat(MAX_SUBJECT);
        m.body = "é".repeat(MAX_BODY);
        assert!(g.admit(&m, 0).is_ok(), "exactly at the caps");
        m.subject.push('x');
        assert_eq!(g.admit(&m, 0), Err(Refusal::TooLong));
        m.subject = "s".into();
        m.body = "b".repeat(MAX_BODY + 1);
        assert_eq!(g.admit(&m, 0), Err(Refusal::TooLong));
    }

    #[test]
    fn one_open_alert_per_sender_until_it_closes() {
        let mut g = gate();
        let m1 = msg("pm-ants", "m1");
        g.admit(&m1, 0).unwrap();
        g.raised(&m1, "a1", 0);
        assert_eq!(g.admit(&msg("pm-ants", "m2"), 5), Err(Refusal::SenderLimit));
        assert!(
            g.admit(&msg("pm-web", "m3"), 5).is_ok(),
            "another sender is unaffected"
        );
        assert_eq!(g.closed("a1").unwrap().message_id, "m1");
        assert!(g.admit(&msg("pm-ants", "m2"), 6).is_ok());
    }

    #[test]
    fn three_a_day_per_sender_on_a_rolling_day() {
        let mut g = gate();
        for i in 0..3u64 {
            let m = msg("pm-ants", &format!("m{i}"));
            g.admit(&m, i * 10).unwrap();
            g.raised(&m, &format!("a{i}"), i * 10);
            g.closed(&format!("a{i}"));
        }
        assert_eq!(g.admit(&msg("pm-ants", "m9"), 100), Err(Refusal::DayLimit));
        assert!(
            g.admit(&msg("pm-ants", "m9"), DAY - 1).is_err(),
            "the first is still inside the day"
        );
        assert!(
            g.admit(&msg("pm-ants", "m9"), DAY).is_ok(),
            "the first has left the day"
        );
    }

    #[test]
    fn three_open_overall() {
        let mut g = gate();
        for s in ["a", "b", "c"] {
            let m = msg(s, s);
            g.admit(&m, 0).unwrap();
            g.raised(&m, s, 0);
        }
        assert_eq!(g.admit(&msg("d", "d"), 1), Err(Refusal::OpenLimit));
    }

    #[test]
    fn a_refused_message_spends_nothing() {
        let mut g = gate();
        let mut bad = msg("pm-ants", "m1");
        bad.urgency_high = false;
        for _ in 0..10 {
            assert_eq!(g.admit(&bad, 0), Err(Refusal::NotHigh));
        }
        assert!(g.admit(&msg("pm-ants", "m2"), 0).is_ok());
        // and an admitted one that was never raised spends nothing either
        assert!(g.admit(&msg("pm-ants", "m2"), 0).is_ok());
    }

    #[test]
    fn polling_gives_up_on_an_alert_after_the_age_limit() {
        let mut g = gate();
        let m = msg("pm-ants", "m1");
        g.raised(&m, "a1", 100);
        let (poll, gone) = g.to_poll(100 + 24 * 3600, 24 * 3600);
        assert_eq!((poll.len(), gone.len()), (1, 0));
        let (poll, gone) = g.to_poll(100 + 24 * 3600 + 1, 24 * 3600);
        assert_eq!((poll.len(), gone.len()), (0, 1));
        assert!(g.open().is_empty());
    }

    #[test]
    fn open_alerts_survive_a_restart_through_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("open.jsonl");
        assert_eq!(
            load_open(&path).unwrap(),
            vec![],
            "no file is no open alerts"
        );
        let mut g = gate();
        for (s, id) in [("pm-ants", "a1"), ("pm-web", "a2")] {
            let m = msg(s, &format!("m-{id}"));
            g.raised(&m, id, 7);
        }
        save_open(&path, &g.open()).unwrap();
        let back = load_open(&path).unwrap();
        assert_eq!(back, g.open());
        let mut g2 = Gate::with_open("ciresnave", BridgeLimits::default(), back);
        assert_eq!(
            g2.admit(&msg("pm-ants", "m9"), 8),
            Err(Refusal::SenderLimit)
        );
        assert_eq!(g2.closed("a1").unwrap().message_id, "m-a1");
    }

    #[test]
    fn a_damaged_file_is_an_error_not_an_empty_list() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("open.jsonl");
        fs::write(&path, "{\"alert_id\":\"a1\"}\n").unwrap();
        assert!(load_open(&path).is_err());
        fs::write(&path, "not json\n").unwrap();
        assert!(load_open(&path).is_err());
    }

    #[test]
    fn the_audit_line_holds_a_length_and_a_hash_never_the_text() {
        let mut m = msg("pm-ants", "m1");
        m.body = "SECRET-TOKEN-12345".into();
        let line = audit_line(AuditEvent::Received, 9, Some(&m), None, None);
        assert!(
            !line.contains("SECRET") && !line.contains("disk full"),
            "{line}"
        );
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["event"], "received");
        assert_eq!(v["sender"], "pm-ants");
        assert_eq!(v["text_len"], "disk full\nSECRET-TOKEN-12345".len());
        assert_eq!(v["text_sha256"].as_str().unwrap().len(), 64);
        let r = audit_line(
            AuditEvent::Refused,
            9,
            Some(&m),
            None,
            Some(Refusal::DayLimit),
        );
        let v: serde_json::Value = serde_json::from_str(&r).unwrap();
        assert_eq!(
            (v["code"].as_str(), v.get("text_sha256")),
            (Some("day-limit"), None)
        );
    }
}
