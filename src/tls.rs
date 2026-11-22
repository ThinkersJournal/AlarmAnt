//! TLS for the public listener: a certificate and key the operator supplies (`ALARMANT_TLS_CERT_FILE`
//! and `ALARMANT_TLS_KEY_FILE`, both or neither). Without them the server speaks plain HTTP and is meant
//! to sit behind a TLS proxy; a phone only enrols against an `https://` address.
//!
//! A home LAN has no public certificate, so `ALARMANT_TLS_SELF_SIGNED_NAMES` (comma-separated IPs and
//! host names) makes the server create a self-signed certificate once in `ALARMANT_DATA_DIR` and reuse it;
//! the enrol link then carries the certificate's SHA-256 (`f=`) and the phone accepts only that certificate
//! (`PATENT-TLS-PIN-READING.md`, constraints T1 to T5: the phone makes no certificate request, nothing is
//! signed or rotated, the pin is one value in the link).
//!
//! Errors are fixed text: they never carry the key, a path's contents or a library message.

use base64::Engine as _;
use httpant_server::{FixedCert, ServerCert, TlsAcceptor};

/// Which files to use: both set, neither set, or a refusal for exactly one.
pub fn paths(
    cert: Option<String>,
    key: Option<String>,
) -> Result<Option<(String, String)>, &'static str> {
    match (cert, key) {
        (None, None) => Ok(None),
        (Some(c), Some(k)) => Ok(Some((c, k))),
        _ => Err("set both ALARMANT_TLS_CERT_FILE and ALARMANT_TLS_KEY_FILE, or neither"),
    }
}

/// The acceptor for a PEM chain (leaf first) and its private key.
pub fn acceptor(chain_pem: &[u8], key_pem: &[u8]) -> Result<TlsAcceptor, &'static str> {
    let cert = ServerCert::from_pem(chain_pem, key_pem).map_err(
        |_| "the TLS certificate and key are not usable (PEM chain, matching private key)",
    )?;
    Ok(TlsAcceptor::new(FixedCert::new(cert)))
}

/// SHA-256 of the first certificate in a PEM chain, as 64 lowercase hex digits.
pub fn fingerprint_hex(chain_pem: &[u8]) -> Result<String, &'static str> {
    let text = std::str::from_utf8(chain_pem).map_err(|_| "the certificate is not PEM text")?;
    let body: String = text
        .split("-----BEGIN CERTIFICATE-----")
        .nth(1)
        .and_then(|rest| rest.split("-----END CERTIFICATE-----").next())
        .ok_or("the certificate is not PEM text")?
        .split_whitespace()
        .collect();
    let der = base64::engine::general_purpose::STANDARD
        .decode(body)
        .map_err(|_| "the certificate is not PEM text")?;
    let digest = ring::digest::digest(&ring::digest::SHA256, &der);
    Ok(digest.as_ref().iter().map(|b| format!("{b:02x}")).collect())
}

const SELF_CERT: &str = "tls-cert.pem";
const SELF_KEY: &str = "tls-key.pem";
const SELF_NAMES: &str = "tls-names.txt";

/// The names as a sorted, lower-case, de-duplicated list (so a reordered or re-cased variable is not a change).
fn name_list(names: &str) -> Vec<String> {
    let mut v: Vec<String> = names
        .split(',')
        .map(|n| n.trim().to_ascii_lowercase())
        .filter(|n| !n.is_empty())
        .collect();
    v.sort();
    v.dedup();
    v
}

/// Writes `bytes` to `path` through a temporary file and a rename, so a crash never leaves a
/// half-written file at `path`. The key file is created owner-only: `0600` on Unix, and on Windows
/// inherited access is removed and only the current user is granted access (ROADMAP A12).
fn write_atomic(path: &std::path::Path, bytes: &[u8], private: bool) -> Result<(), &'static str> {
    use std::io::Write;
    let tmp = path.with_extension("tmp");
    let _ = std::fs::remove_file(&tmp);
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    if private {
        std::os::unix::fs::OpenOptionsExt::mode(&mut o, 0o600);
    }
    #[cfg(not(unix))]
    let _ = private;
    o.open(&tmp)
        .and_then(|mut f| {
            // The file is empty here: narrow its access before any key byte is written to it.
            if private {
                owner_only_windows(&tmp)?;
            }
            f.write_all(bytes).and_then(|()| f.sync_all())
        })
        .and_then(|()| std::fs::rename(&tmp, path))
        .map_err(|_| {
            let _ = std::fs::remove_file(&tmp);
            "a TLS file could not be written to the data directory (on Windows: or its access could not be limited to the current user)"
        })
}

/// ROADMAP A12: on Windows, drops inherited access from `path` and grants Full control to the current
/// user only (`icacls`, so no unsafe code). A failure is an error, never a key left readable by others.
/// Nothing to do elsewhere (Unix creates the file `0600`).
#[cfg(windows)]
fn owner_only_windows(path: &std::path::Path) -> std::io::Result<()> {
    let user = std::env::var("USERNAME").map_err(|_| std::io::Error::other("no USERNAME"))?;
    let who = match std::env::var("USERDOMAIN") {
        Ok(d) if !d.is_empty() => format!("{d}\\{user}"),
        _ => user,
    };
    let out = std::process::Command::new("icacls")
        .arg(path)
        .args(["/inheritance:r", "/grant:r"])
        .arg(format!("{who}:F"))
        .output()?;
    if out.status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other("icacls failed"))
    }
}

#[cfg(not(windows))]
fn owner_only_windows(_: &std::path::Path) -> std::io::Result<()> {
    Ok(())
}

/// The self-signed pair in `dir`, for exactly `names` (IPs become IP subject alternative names).
///
/// * Complete pair for the same names: reused, so the fingerprint and every phone's pin stay valid.
/// * Complete pair for **different** names: refused with a message that says what to delete, never
///   served silently with a certificate the phone's hostname check will reject.
/// * Nothing, or an interrupted earlier run (the certificate is written last, so a missing
///   certificate means the run did not finish): a new pair is made.
/// * A certificate without its key: refused (it cannot be repaired from here).
///
/// Returns the two file paths.
pub fn self_signed_in(
    dir: &std::path::Path,
    names: &str,
) -> Result<(String, String), &'static str> {
    let cert = dir.join(SELF_CERT);
    let key = dir.join(SELF_KEY);
    let names_file = dir.join(SELF_NAMES);
    let wanted = name_list(names);
    if wanted.is_empty() {
        return Err("ALARMANT_TLS_SELF_SIGNED_NAMES lists no name");
    }
    if cert.exists() {
        if !key.exists() {
            return Err(
                "tls-cert.pem exists without tls-key.pem: delete tls-cert.pem to make a new pair (phones must enrol again)",
            );
        }
        let had = std::fs::read_to_string(&names_file).map(|t| name_list(&t.replace('\n', ",")));
        if had.as_deref().ok() != Some(wanted.as_slice()) {
            return Err(
                "ALARMANT_TLS_SELF_SIGNED_NAMES differs from the names the certificate was made for: delete tls-cert.pem, tls-key.pem and tls-names.txt to make a new pair (phones must enrol again)",
            );
        }
    } else {
        // Nothing yet, or an interrupted run: start clean.
        let _ = std::fs::remove_file(&key);
        let _ = std::fs::remove_file(&names_file);
        let made = rcgen::generate_simple_self_signed(wanted.clone())
            .map_err(|_| "a self-signed certificate could not be made")?;
        write_atomic(&key, made.signing_key.serialize_pem().as_bytes(), true)?;
        write_atomic(&names_file, wanted.join("\n").as_bytes(), false)?;
        write_atomic(&cert, made.cert.pem().as_bytes(), false)?; // last: its presence means complete
    }
    let s = |p: std::path::PathBuf| {
        p.to_str()
            .map(str::to_owned)
            .ok_or("the data directory path is not text")
    };
    Ok((s(cert)?, s(key)?))
}

/// `alarmant://enroll?u=<base>&c=<code>[&f=<sha256 hex>]`, with the base URL percent-encoded.
pub fn enrol_link(base: &str, code: &str, fingerprint: Option<&str>) -> String {
    let mut enc = String::new();
    for b in base.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                enc.push(b as char)
            }
            _ => enc.push_str(&format!("%{b:02X}")),
        }
    }
    let mut link = format!("alarmant://enroll?u={enc}&c={code}");
    if let Some(f) = fingerprint {
        link.push_str("&f=");
        link.push_str(f);
    }
    link
}

/// The enrolment code in the admin reply: the one line that is exactly 32 lowercase hex digits
/// (not a position in the text, so a reworded reply cannot make the link carry the wrong line).
pub fn code_in_reply(text: &str) -> Option<&str> {
    text.lines().map(str::trim).find(|l| {
        l.len() == 32
            && l.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

/// The host of an `https://host[:port]/...` address, lower-case, without a port or brackets.
fn https_host(base: &str) -> Option<String> {
    let rest = base.strip_prefix("https://")?;
    let auth = rest.split(['/', '?', '#']).next()?;
    let auth = auth.rsplit_once('@').map_or(auth, |(_, h)| h);
    let host = if let Some(h) = auth.strip_prefix('[') {
        h.split(']').next()?
    } else {
        auth.rsplit_once(':').map_or(auth, |(h, _)| h)
    };
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

/// The enrol link for the operator: `base` must be `https://`. With `self_signed_dir` (the server makes
/// its own certificate) the pair must be readable there and `base`'s host must be one of the names the
/// certificate was made for, or the phone would fail the handshake and the one-time code would be
/// wasted; the pin is added. Without it nothing is pinned and the system trust store applies.
pub fn link_for(
    base: &str,
    code: &str,
    self_signed_dir: Option<&std::path::Path>,
) -> Result<String, String> {
    let Some(host) = https_host(base) else {
        return Err(
            "ALARMANT_PUBLIC_URL must be an https:// address (the phone rejects anything else)"
                .to_owned(),
        );
    };
    let pin = match self_signed_dir {
        None => None,
        Some(dir) => {
            let pem = std::fs::read(dir.join(SELF_CERT)).map_err(|_| {
                "ALARMANT_TLS_SELF_SIGNED_NAMES is set but tls-cert.pem cannot be read from ALARMANT_DATA_DIR (start the server once first)".to_owned()
            })?;
            let names = std::fs::read_to_string(dir.join(SELF_NAMES))
                .map(|t| name_list(&t.replace('\n', ",")))
                .map_err(|_| "tls-names.txt cannot be read from ALARMANT_DATA_DIR".to_owned())?;
            if !names.contains(&host) {
                return Err("the host in ALARMANT_PUBLIC_URL is not one of the names the certificate was made for (tls-names.txt): the phone would refuse it".to_owned());
            }
            Some(fingerprint_hex(&pem).map_err(str::to_owned)?)
        }
    };
    Ok(enrol_link(base, code, pin.as_deref()))
}

/// Read both files and build the acceptor.
pub fn acceptor_from_files(cert_path: &str, key_path: &str) -> Result<TlsAcceptor, &'static str> {
    let chain = std::fs::read(cert_path).map_err(|_| "ALARMANT_TLS_CERT_FILE cannot be read")?;
    let key = std::fs::read(key_path).map_err(|_| "ALARMANT_TLS_KEY_FILE cannot be read")?;
    acceptor(&chain, &key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair() -> (String, String) {
        let ck = rcgen::generate_simple_self_signed(vec!["alarmant.test".to_owned()]).unwrap();
        (ck.cert.pem(), ck.signing_key.serialize_pem())
    }

    #[test]
    fn the_fingerprint_is_the_sha256_of_the_certificate_der() {
        let (c, _) = pair();
        let f = fingerprint_hex(c.as_bytes()).unwrap();
        assert_eq!(f.len(), 64);
        assert!(
            f.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        );
        assert_eq!(f, fingerprint_hex(c.as_bytes()).unwrap());
        let (other, _) = pair();
        assert_ne!(f, fingerprint_hex(other.as_bytes()).unwrap());
        assert!(fingerprint_hex(b"nope").is_err());
    }

    /// ROADMAP A12: on Windows the key file's access list names the current user and no one else
    /// (no inherited `Users`, `Everyone` or `Authenticated Users` entry).
    #[cfg(windows)]
    #[test]
    fn the_private_key_is_readable_by_the_current_user_only_on_windows() {
        let d = tempfile::tempdir().unwrap();
        let (_, key) = self_signed_in(d.path(), "127.0.0.1").unwrap();
        let out = std::process::Command::new("icacls")
            .arg(&key)
            .output()
            .unwrap();
        let acl = String::from_utf8_lossy(&out.stdout).to_string();
        let user = std::env::var("USERNAME").unwrap().to_lowercase();
        let entries: Vec<&str> = acl.lines().filter(|l| l.contains(":(")).collect();
        assert_eq!(entries.len(), 1, "one access entry only: {acl}");
        // the first line starts with the file's own path (which has `Users` in it): look past it
        let entry = entries[0].replace(&key, "").to_lowercase();
        assert!(entry.contains(&user), "{acl}");
        for other in [
            "users",
            "everyone",
            "authenticated users",
            "system",
            "administrators",
        ] {
            assert!(!entry.contains(other), "{other} in {acl}");
        }
        // the key still works after the narrowing
        assert!(
            acceptor_from_files(&d.path().join("tls-cert.pem").display().to_string(), &key).is_ok()
        );
    }

    #[test]
    fn a_self_signed_pair_is_made_once_and_reused_with_the_same_fingerprint() {
        let d = tempfile::tempdir().unwrap();
        let (c1, k1) = self_signed_in(d.path(), "127.0.0.1, alarmant.test").unwrap();
        let f1 = fingerprint_hex(&std::fs::read(&c1).unwrap()).unwrap();
        let (c2, _) = self_signed_in(d.path(), "alarmant.test,127.0.0.1").unwrap();
        assert_eq!(f1, fingerprint_hex(&std::fs::read(&c2).unwrap()).unwrap());
        assert!(acceptor_from_files(&c1, &k1).is_ok());
        // the IP name is an IP subject alternative name (4 raw bytes in the DER), not only text
        let b64: String = std::fs::read_to_string(&c1)
            .unwrap()
            .lines()
            .filter(|l| !l.starts_with("-----"))
            .collect();
        let der = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .unwrap();
        assert!(der.windows(4).any(|w| w == [127, 0, 0, 1]));
    }

    #[test]
    fn no_names_or_a_certificate_without_its_key_is_refused() {
        let d = tempfile::tempdir().unwrap();
        assert!(self_signed_in(d.path(), " , ").is_err());
        std::fs::write(d.path().join("tls-cert.pem"), "x").unwrap();
        assert!(self_signed_in(d.path(), "a.test").is_err());
    }

    #[test]
    fn a_key_left_by_an_interrupted_run_is_replaced_not_fatal() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("tls-key.pem"), "half written").unwrap();
        std::fs::write(d.path().join("tls-names.txt"), "stale").unwrap();
        let (c, k) = self_signed_in(d.path(), "a.test").unwrap();
        assert!(acceptor_from_files(&c, &k).is_ok());
        assert!(!d.path().join("tls-cert.tmp").exists());
    }

    #[test]
    fn changed_names_are_refused_not_served_with_the_old_certificate() {
        let d = tempfile::tempdir().unwrap();
        self_signed_in(d.path(), "192.168.1.10, b.test").unwrap();
        // same set in another order and spacing: reused
        assert!(self_signed_in(d.path(), "b.test,192.168.1.10").is_ok());
        let e = self_signed_in(d.path(), "192.168.1.20").unwrap_err();
        assert!(e.contains("differs") && e.contains("delete"));
    }

    #[test]
    fn the_code_is_found_by_shape_not_by_line_position() {
        let code = "0123456789abcdef0123456789abcdef";
        let reply = format!(
            "Enrolment code (shown once, valid 10 minutes, usable once):
{code}
"
        );
        assert_eq!(code_in_reply(&reply), Some(code));
        assert_eq!(
            code_in_reply(&format!(
                "a note first
and another
  {code}  
"
            )),
            Some(code)
        );
        assert_eq!(
            code_in_reply(
                "Device removed.
No code here
"
            ),
            None
        );
        assert_eq!(code_in_reply("0123456789ABCDEF0123456789abcdef"), None);
    }

    #[test]
    fn the_link_encodes_the_base_and_adds_the_pin_only_when_given() {
        assert_eq!(
            enrol_link("https://10.0.0.5:8443", "abc", None),
            "alarmant://enroll?u=https%3A%2F%2F10.0.0.5%3A8443&c=abc"
        );
        assert!(enrol_link("https://h", "abc", Some("ff")).ends_with("&c=abc&f=ff"));
    }

    #[test]
    fn the_operator_link_needs_https_and_pins_only_a_self_signed_pair() {
        let d = tempfile::tempdir().unwrap();
        let (c, _) = self_signed_in(d.path(), "10.0.0.5, Alarm.Home").unwrap();
        let f = fingerprint_hex(&std::fs::read(c).unwrap()).unwrap();
        let pinned = link_for("https://10.0.0.5:8443", "abc", Some(d.path())).unwrap();
        assert!(pinned.ends_with(&format!("&f={f}")));
        // the host may be written in another case; a name the certificate does not cover is refused
        assert!(link_for("https://ALARM.home/", "abc", Some(d.path())).is_ok());
        assert!(link_for("https://10.0.0.99:8443", "abc", Some(d.path())).is_err());
        assert!(link_for("https://user@10.0.0.5", "abc", Some(d.path())).is_ok());
        // a real certificate: a stale tls-cert.pem in the directory is ignored
        assert!(
            !link_for("https://h.example", "abc", None)
                .unwrap()
                .contains("&f=")
        );
        assert!(link_for("http://h", "abc", None).is_err());
        assert!(
            link_for(
                "https://h",
                "abc",
                Some(tempfile::tempdir().unwrap().path())
            )
            .is_err()
        );
        assert_eq!(https_host("https://[::1]:8443/x"), Some("::1".to_owned()));
        assert_eq!(https_host("http://h"), None);
    }

    #[test]
    fn both_or_neither_path_is_accepted_and_one_alone_refused() {
        assert_eq!(paths(None, None), Ok(None));
        assert_eq!(
            paths(Some("c".into()), Some("k".into())),
            Ok(Some(("c".into(), "k".into())))
        );
        assert!(paths(Some("c".into()), None).is_err());
        assert!(paths(None, Some("k".into())).is_err());
    }

    #[test]
    fn a_matching_pair_builds_an_acceptor() {
        let (c, k) = pair();
        assert!(acceptor(c.as_bytes(), k.as_bytes()).is_ok());
    }

    #[test]
    fn a_mismatched_key_is_refused_and_the_error_carries_no_key_text() {
        let (c, _) = pair();
        let (_, other_key) = pair();
        let e = acceptor(c.as_bytes(), other_key.as_bytes()).unwrap_err();
        assert!(!e.contains("PRIVATE") && !other_key.lines().nth(1).is_some_and(|l| e.contains(l)));
    }

    #[tokio::test]
    async fn the_acceptor_binds_a_listener() {
        let (c, k) = pair();
        let running = httpant_server::Server::new()
            .tls(acceptor(c.as_bytes(), k.as_bytes()).unwrap())
            .bind("127.0.0.1:0")
            .await;
        assert!(running.is_ok());
    }

    #[test]
    fn garbage_and_missing_files_are_refused_with_fixed_text() {
        assert!(acceptor(b"not pem", b"not pem").is_err());
        let e = acceptor_from_files("/no/such/cert", "/no/such/key").unwrap_err();
        assert!(!e.contains("/no/such"));
    }
}
