//! Registration slice (g): the stores on disk, through PersistAnt (DESIGN-REGISTRATION.md section 5).
//!
//! [`PersistedDevices`], [`PersistedAlerts`] and [`PersistedEnrollments`] are the in-memory stores
//! with a write-through snapshot: every change is written with PersistAnt's atomic replace, under
//! a lock, before the call returns, and the files are read once when the store is opened. The
//! service is the only writer (one process), so the memory copy is the truth between writes.
//!
//! What lands on disk: ack tokens and enrolment codes only as SHA-256 digests; the FCM push token
//! in clear (it is needed to send), so the data directory must be readable by the service account
//! only (DESIGN-REGISTRATION ruling 3, until SecretAnt). Nothing here logs, and no error text
//! carries a stored value.
//!
//! A store that cannot write stops the service (a panic with fixed text) rather than go on with a
//! state that a restart would silently undo: a revocation that did not reach the disk must not
//! look done.

use std::path::Path;
use std::sync::{Arc, Mutex};

use persistant::blocking::Store;
use persistant::{Config, Error, Need, Needs};
use serde_json::{Value, json};

use crate::AlertState;
use crate::ack::{AckToken, AlertRecord, AlertStore, AlertText, MemoryStore};
use crate::enroll::{Contact, EnrollError, Enrollment, EnrollmentStore, MemoryEnrollments};
use crate::fcm::DeviceToken;
use crate::raise::{Device, DeviceRegistry};
use crate::register::{DeviceStore, MemoryDevices};

/// Why a data directory could not be opened. Fixed text; never a stored value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersistError {
    /// The directory or the storage backend could not be opened.
    Open,
    /// A stored file exists but is not in the expected form; it is not overwritten.
    Corrupt,
}

impl std::fmt::Display for PersistError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            PersistError::Open => "the data directory could not be opened",
            PersistError::Corrupt => "a stored file is not in the expected form",
        })
    }
}

impl std::error::Error for PersistError {}

/// The data directory: one JSON file per store, each replaced atomically.
pub struct Disk {
    store: Store,
}

impl std::fmt::Debug for Disk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Disk")
    }
}

impl Disk {
    /// Opens (creating if needed) `dir`; scratch space for atomic replace is `dir/.tmp`.
    pub fn open(dir: &Path) -> Result<Arc<Disk>, PersistError> {
        let root = dir.join("data");
        let scratch = dir.join(".tmp");
        std::fs::create_dir_all(&root).map_err(|_| PersistError::Open)?;
        std::fs::create_dir_all(&scratch).map_err(|_| PersistError::Open)?;
        let needs = Needs::new()
            .with(Need::Read)
            .with(Need::Write)
            .with(Need::Delete)
            .with(Need::AtomicReplace);
        let store = Store::open(
            Config::Fs {
                root,
                atomic_write_dir: Some(scratch),
            },
            needs,
        )
        .map_err(|_| PersistError::Open)?;
        Ok(Arc::new(Disk { store }))
    }

    fn load(&self, key: &str) -> Result<Option<Value>, PersistError> {
        match self.store.read(key) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|_| PersistError::Corrupt),
            Err(Error::NotFound(_)) => Ok(None),
            Err(_) => Err(PersistError::Open),
        }
    }

    fn save(&self, key: &str, v: &Value) {
        let bytes = serde_json::to_vec(v).expect("a JSON value always serialises");
        if self.store.replace(key, bytes).is_err() {
            panic!("the data directory could not be written; stopping rather than lose a change");
        }
    }
}

fn token(v: &Value, name: &str) -> Option<AckToken> {
    AckToken::from_digest_hex(v.get(name)?.as_str()?)
}

fn device_value(d: &Device) -> Value {
    json!({ "fcm_token": d.push_token.expose(), "ack_digest": d.ack_token.digest_hex() })
}

fn device_from(v: &Value) -> Option<Device> {
    if v.is_null() {
        return None;
    }
    Some(Device {
        push_token: DeviceToken::new(v.get("fcm_token")?.as_str()?),
        ack_token: token(v, "ack_digest")?,
    })
}

/// [`DeviceStore`] on disk (`devices.json`).
#[derive(Debug)]
pub struct PersistedDevices {
    disk: Arc<Disk>,
    mem: MemoryDevices,
    write: Mutex<()>,
}

const DEVICES: &str = "devices.json";
const ALERTS: &str = "alerts.json";
const ENROLLMENTS: &str = "enrollments.json";

impl PersistedDevices {
    pub fn open(disk: &Arc<Disk>) -> Result<Self, PersistError> {
        let mem = MemoryDevices::default();
        if let Some(v) = disk.load(DEVICES)? {
            let slots = v
                .as_array()
                .filter(|a| a.len() == 2)
                .ok_or(PersistError::Corrupt)?;
            for (contact, slot) in [Contact::First, Contact::Second].into_iter().zip(slots) {
                if !slot.is_null() {
                    mem.set_for(contact, device_from(slot).ok_or(PersistError::Corrupt)?);
                }
            }
        }
        Ok(PersistedDevices {
            disk: Arc::clone(disk),
            mem,
            write: Mutex::new(()),
        })
    }

    fn save(&self) {
        let slots: Vec<Value> = self
            .mem
            .snapshot()
            .iter()
            .map(|d| d.as_ref().map_or(Value::Null, device_value))
            .collect();
        self.disk.save(DEVICES, &Value::Array(slots));
    }
}

impl DeviceRegistry for PersistedDevices {
    fn device(&self) -> Option<Device> {
        self.mem.device()
    }

    fn second_device(&self) -> Option<Device> {
        self.mem.second_device()
    }
}

impl DeviceStore for PersistedDevices {
    fn set_for(&self, contact: Contact, d: Device) {
        let _w = self.write.lock().unwrap_or_else(|p| p.into_inner());
        self.mem.set_for(contact, d);
        self.save();
    }

    fn replace_push_token(&self, presented_ack: &str, token: DeviceToken) -> bool {
        let _w = self.write.lock().unwrap_or_else(|p| p.into_inner());
        let done = self.mem.replace_push_token(presented_ack, token);
        if done {
            self.save();
        }
        done
    }

    fn take_for(&self, contact: Contact) -> Option<Device> {
        let _w = self.write.lock().unwrap_or_else(|p| p.into_inner());
        let taken = self.mem.take_for(contact);
        if taken.is_some() {
            self.save();
        }
        taken
    }
}

fn state_name(s: AlertState) -> &'static str {
    match s {
        AlertState::Sent => "sent",
        AlertState::Escalated => "escalated",
        AlertState::Acked => "acked",
        AlertState::Unresolved => "unresolved",
    }
}

fn state_from(s: &str) -> Option<AlertState> {
    Some(match s {
        "sent" => AlertState::Sent,
        "escalated" => AlertState::Escalated,
        "acked" => AlertState::Acked,
        "unresolved" => AlertState::Unresolved,
        _ => return None,
    })
}

fn alert_value(r: &AlertRecord) -> Value {
    json!({
        "alert_id": r.alert_id,
        "state": state_name(r.state),
        "sent_at": r.sent_at,
        "acked_at": r.acked_at,
        "ack_digest": r.ack_token.digest_hex(),
        "second_ack_digest": r.second_ack_token.as_ref().map(AckToken::digest_hex),
        "text": r.text.as_ref().map(|t| json!({ "title": t.title, "body": t.body })),
    })
}

fn alert_from(v: &Value) -> Option<AlertRecord> {
    let second = match v.get("second_ack_digest")? {
        Value::Null => None,
        s => Some(AckToken::from_digest_hex(s.as_str()?)?),
    };
    let text = match v.get("text")? {
        Value::Null => None,
        t => Some(AlertText {
            title: t.get("title")?.as_str()?.to_owned(),
            body: t.get("body")?.as_str()?.to_owned(),
        }),
    };
    Some(AlertRecord {
        alert_id: v.get("alert_id")?.as_str()?.to_owned(),
        state: state_from(v.get("state")?.as_str()?)?,
        sent_at: v.get("sent_at")?.as_u64()?,
        acked_at: match v.get("acked_at")? {
            Value::Null => None,
            a => Some(a.as_u64()?),
        },
        ack_token: token(v, "ack_digest")?,
        second_ack_token: second,
        text,
    })
}

/// [`AlertStore`] on disk (`alerts.json`).
#[derive(Debug)]
pub struct PersistedAlerts {
    disk: Arc<Disk>,
    mem: MemoryStore,
    write: Mutex<()>,
}

impl PersistedAlerts {
    pub fn open(disk: &Arc<Disk>) -> Result<Self, PersistError> {
        let mem = MemoryStore::default();
        if let Some(v) = disk.load(ALERTS)? {
            for r in v.as_array().ok_or(PersistError::Corrupt)? {
                mem.insert(alert_from(r).ok_or(PersistError::Corrupt)?);
            }
        }
        Ok(PersistedAlerts {
            disk: Arc::clone(disk),
            mem,
            write: Mutex::new(()),
        })
    }

    fn save(&self) {
        let all: Vec<Value> = self.mem.snapshot().iter().map(alert_value).collect();
        self.disk.save(ALERTS, &Value::Array(all));
    }
}

impl AlertStore for PersistedAlerts {
    fn insert(&self, record: AlertRecord) {
        let _w = self.write.lock().unwrap_or_else(|p| p.into_inner());
        self.mem.insert(record);
        self.save();
    }

    fn get(&self, alert_id: &str) -> Option<AlertRecord> {
        self.mem.get(alert_id)
    }

    fn remove(&self, alert_id: &str) {
        let _w = self.write.lock().unwrap_or_else(|p| p.into_inner());
        self.mem.remove(alert_id);
        self.save();
    }

    fn mark_acked(&self, alert_id: &str, at: u64) -> Option<bool> {
        let _w = self.write.lock().unwrap_or_else(|p| p.into_inner());
        let r = self.mem.mark_acked(alert_id, at);
        if r == Some(true) {
            self.save();
        }
        r
    }

    fn void_ack_tokens(&self, revoked: &AckToken) -> usize {
        let _w = self.write.lock().unwrap_or_else(|p| p.into_inner());
        let n = self.mem.void_ack_tokens(revoked);
        if n > 0 {
            self.save();
        }
        n
    }

    fn set_second_ack_token(&self, alert_id: &str, token: AckToken) -> bool {
        let _w = self.write.lock().unwrap_or_else(|p| p.into_inner());
        let ok = self.mem.set_second_ack_token(alert_id, token);
        if ok {
            self.save();
        }
        ok
    }
}

/// [`EnrollmentStore`] on disk (`enrollments.json`).
#[derive(Debug)]
pub struct PersistedEnrollments {
    disk: Arc<Disk>,
    mem: MemoryEnrollments,
    write: Mutex<()>,
}

impl PersistedEnrollments {
    pub fn open(disk: &Arc<Disk>) -> Result<Self, PersistError> {
        let mem = MemoryEnrollments::default();
        if let Some(v) = disk.load(ENROLLMENTS)? {
            for e in v.as_array().ok_or(PersistError::Corrupt)? {
                mem.set(Enrollment::from_value(e).ok_or(PersistError::Corrupt)?);
            }
        }
        Ok(PersistedEnrollments {
            disk: Arc::clone(disk),
            mem,
            write: Mutex::new(()),
        })
    }

    fn save(&self) {
        let all: Vec<Value> = self
            .mem
            .snapshot()
            .iter()
            .map(Enrollment::to_value)
            .collect();
        self.disk.save(ENROLLMENTS, &Value::Array(all));
    }
}

impl EnrollmentStore for PersistedEnrollments {
    fn set(&self, e: Enrollment) {
        let _w = self.write.lock().unwrap_or_else(|p| p.into_inner());
        self.mem.set(e);
        self.save();
    }

    fn consume(&self, presented: &str, now: u64) -> Result<Contact, EnrollError> {
        let _w = self.write.lock().unwrap_or_else(|p| p.into_inner());
        let before = self.mem.snapshot().len();
        let r = self.mem.consume(presented, now);
        // A right code (used or expired) removes its entry; a wrong one changes nothing.
        if self.mem.snapshot().len() != before {
            self.save();
        }
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enroll::new_enrollment_for;
    use crate::revoke::revoke_contact;

    fn dev(push: &str, ack: &str) -> Device {
        Device {
            push_token: DeviceToken::new(push),
            ack_token: AckToken::new(ack),
        }
    }

    fn all_files(dir: &Path) -> String {
        let mut out = String::new();
        for d in ["data", ".tmp"] {
            if let Ok(rd) = std::fs::read_dir(dir.join(d)) {
                for e in rd.flatten() {
                    out.push_str(&std::fs::read_to_string(e.path()).unwrap_or_default());
                }
            }
        }
        out
    }

    #[test]
    fn devices_survive_a_restart_with_both_slots_and_token_rotation() {
        let dir = tempfile::tempdir().unwrap();
        {
            let disk = Disk::open(dir.path()).unwrap();
            let d = PersistedDevices::open(&disk).unwrap();
            d.set_for(Contact::First, dev("fcm-1", "ack-1"));
            d.set_for(Contact::Second, dev("fcm-2", "ack-2"));
            assert!(d.replace_push_token("ack-2", DeviceToken::new("fcm-2b")));
        }
        let disk = Disk::open(dir.path()).unwrap();
        let d = PersistedDevices::open(&disk).unwrap();
        assert!(d.device().unwrap().ack_token.matches("ack-1"));
        assert_eq!(d.device().unwrap().push_token.expose(), "fcm-1");
        assert!(d.second_device().unwrap().ack_token.matches("ack-2"));
        assert_eq!(d.second_device().unwrap().push_token.expose(), "fcm-2b");
    }

    #[test]
    fn a_revocation_is_still_a_revocation_after_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        {
            let disk = Disk::open(dir.path()).unwrap();
            let (d, a) = (
                PersistedDevices::open(&disk).unwrap(),
                PersistedAlerts::open(&disk).unwrap(),
            );
            d.set_for(Contact::First, dev("fcm-1", "ack-1"));
            a.insert(AlertRecord::sent("a-1", 10, AckToken::new("ack-1")));
            let r = revoke_contact(&d, &a, Contact::First);
            assert!(r.had_device);
            assert_eq!(r.alerts_voided, 1);
        }
        let disk = Disk::open(dir.path()).unwrap();
        let (d, a) = (
            PersistedDevices::open(&disk).unwrap(),
            PersistedAlerts::open(&disk).unwrap(),
        );
        assert!(d.device().is_none());
        assert!(!a.get("a-1").unwrap().ack_token.matches("ack-1"));
    }

    #[test]
    fn alerts_round_trip_every_field() {
        let dir = tempfile::tempdir().unwrap();
        let mut rec = AlertRecord::sent("a-1", 10, AckToken::new("one"));
        rec.text = Some(AlertText {
            title: "DB down".into(),
            body: "prod-1".into(),
        });
        {
            let disk = Disk::open(dir.path()).unwrap();
            let a = PersistedAlerts::open(&disk).unwrap();
            a.insert(rec.clone());
            assert!(a.set_second_ack_token("a-1", AckToken::new("two")));
            assert_eq!(a.mark_acked("a-1", 99), Some(true));
        }
        let disk = Disk::open(dir.path()).unwrap();
        let got = PersistedAlerts::open(&disk).unwrap().get("a-1").unwrap();
        assert_eq!(got.state, AlertState::Acked);
        assert_eq!(got.acked_at, Some(99));
        assert_eq!(got.sent_at, 10);
        assert_eq!(got.text, rec.text);
        assert!(got.ack_token.matches("one"));
        assert!(got.second_ack_token.unwrap().matches("two"));
    }

    #[test]
    fn a_pending_code_survives_a_restart_and_is_spent_once() {
        let dir = tempfile::tempdir().unwrap();
        let code = {
            let disk = Disk::open(dir.path()).unwrap();
            let e = PersistedEnrollments::open(&disk).unwrap();
            let (code, enrollment) = new_enrollment_for(Contact::Second, 1000);
            e.set(enrollment);
            code
        };
        let disk = Disk::open(dir.path()).unwrap();
        let e = PersistedEnrollments::open(&disk).unwrap();
        assert_eq!(e.consume("wrong", 1001), Err(EnrollError::Unknown));
        assert_eq!(e.consume(&code, 1001), Ok(Contact::Second));
        let e = PersistedEnrollments::open(&disk).unwrap();
        assert_eq!(
            e.consume(&code, 1002),
            Err(EnrollError::Unknown),
            "spent stays spent"
        );
    }

    #[test]
    fn no_ack_token_or_enrolment_code_is_on_disk_in_clear() {
        let dir = tempfile::tempdir().unwrap();
        let disk = Disk::open(dir.path()).unwrap();
        let (d, a, e) = (
            PersistedDevices::open(&disk).unwrap(),
            PersistedAlerts::open(&disk).unwrap(),
            PersistedEnrollments::open(&disk).unwrap(),
        );
        d.set_for(Contact::First, dev("fcm-visible", "SECRET-ack-token"));
        a.insert(AlertRecord::sent(
            "a-1",
            1,
            AckToken::new("SECRET-ack-token"),
        ));
        let (code, enrollment) = new_enrollment_for(Contact::First, 1);
        e.set(enrollment);
        let files = all_files(dir.path());
        assert!(
            files.contains("fcm-visible"),
            "the push token is needed in clear"
        );
        assert!(!files.contains("SECRET-ack-token"));
        assert!(!files.contains(&code));
    }

    #[test]
    fn a_corrupt_file_is_refused_and_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        {
            Disk::open(dir.path()).unwrap();
        }
        let path = dir.path().join("data").join("devices.json");
        std::fs::write(&path, b"{ not json").unwrap();
        let disk = Disk::open(dir.path()).unwrap();
        assert_eq!(
            PersistedDevices::open(&disk).err(),
            Some(PersistError::Corrupt)
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"{ not json");
        std::fs::write(&path, br#"[{"fcm_token":"x","ack_digest":"nothex"},null]"#).unwrap();
        assert_eq!(
            PersistedDevices::open(&disk).err(),
            Some(PersistError::Corrupt)
        );
    }

    #[test]
    fn an_empty_directory_opens_empty() {
        let dir = tempfile::tempdir().unwrap();
        let disk = Disk::open(dir.path()).unwrap();
        assert!(PersistedDevices::open(&disk).unwrap().device().is_none());
        assert!(PersistedAlerts::open(&disk).unwrap().get("x").is_none());
        assert_eq!(
            PersistedEnrollments::open(&disk).unwrap().consume("x", 1),
            Err(EnrollError::Unknown)
        );
    }
}
