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
pub fn paths(cert: Option<String>, key: Option<String>) -> Result<Option<(String, String)>, &'static str> {
    match (cert, key) {
        (None, None) => Ok(None),
        (Some(c), Some(k)) => Ok(Some((c, k))),
        _ => Err("set both ALARMANT_TLS_CERT_FILE and ALARMANT_TLS_KEY_FILE, or neither"),
    }
}

/// The acceptor for a PEM chain (leaf first) and its private key.
pub fn acceptor(chain_pem: &[u8], key_pem: &[u8]) -> Result<TlsAcceptor, &'static str> {
    let cert = ServerCert::from_pem(chain_pem, key_pem)
        .map_err(|_| "the TLS certificate and key are not usable (PEM chain, matching private key)")?;
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

/// The self-signed pair in `dir`: reused when both files exist, created (with every name as a
/// subject alternative name, IPs as IP entries) when neither does. Returns the two file paths.
pub fn self_signed_in(dir: &std::path::Path, names: &str) -> Result<(String, String), &'static str> {
    let cert = dir.join(SELF_CERT);
    let key = dir.join(SELF_KEY);
    match (cert.exists(), key.exists()) {
        (true, true) => {}
        (false, false) => {
            let names: Vec<String> = names
                .split(',')
                .map(|n| n.trim().to_owned())
                .filter(|n| !n.is_empty())
                .collect();
            if names.is_empty() {
                return Err("ALARMANT_TLS_SELF_SIGNED_NAMES lists no name");
            }
            let made = rcgen::generate_simple_self_signed(names)
                .map_err(|_| "a self-signed certificate could not be made")?;
            write_private(&key, made.signing_key.serialize_pem().as_bytes())?;
            std::fs::write(&cert, made.cert.pem()).map_err(|_| "the certificate could not be written")?;
        }
        _ => return Err("only one of tls-cert.pem and tls-key.pem exists in the data directory"),
    }
    let s = |p: std::path::PathBuf| p.to_str().map(str::to_owned).ok_or("the data directory path is not text");
    Ok((s(cert)?, s(key)?))
}

fn write_private(path: &std::path::Path, bytes: &[u8]) -> Result<(), &'static str> {
    use std::io::Write;
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut o, 0o600);
    o.open(path)
        .and_then(|mut f| f.write_all(bytes))
        .map_err(|_| "the private key could not be written")
}

/// `alarmant://enroll?u=<base>&c=<code>[&f=<sha256 hex>]`, with the base URL percent-encoded.
pub fn enrol_link(base: &str, code: &str, fingerprint: Option<&str>) -> String {
    let mut enc = String::new();
    for b in base.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => enc.push(b as char),
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
        assert!(f.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
        assert_eq!(f, fingerprint_hex(c.as_bytes()).unwrap());
        let (other, _) = pair();
        assert_ne!(f, fingerprint_hex(other.as_bytes()).unwrap());
        assert!(fingerprint_hex(b"nope").is_err());
    }

    #[test]
    fn a_self_signed_pair_is_made_once_and_reused_with_the_same_fingerprint() {
        let d = tempfile::tempdir().unwrap();
        let (c1, k1) = self_signed_in(d.path(), "127.0.0.1, alarmant.test").unwrap();
        let f1 = fingerprint_hex(&std::fs::read(&c1).unwrap()).unwrap();
        let (c2, _) = self_signed_in(d.path(), "other.example").unwrap();
        assert_eq!(f1, fingerprint_hex(&std::fs::read(&c2).unwrap()).unwrap());
        assert!(acceptor_from_files(&c1, &k1).is_ok());
        // the IP name is an IP subject alternative name (4 raw bytes in the DER), not only text
        let b64: String = std::fs::read_to_string(&c1).unwrap().lines().filter(|l| !l.starts_with("-----")).collect();
        let der = base64::engine::general_purpose::STANDARD.decode(b64).unwrap();
        assert!(der.windows(4).any(|w| w == [127, 0, 0, 1]));
    }

    #[test]
    fn half_a_pair_or_no_names_is_refused() {
        let d = tempfile::tempdir().unwrap();
        assert!(self_signed_in(d.path(), " , ").is_err());
        std::fs::write(d.path().join("tls-cert.pem"), "x").unwrap();
        assert!(self_signed_in(d.path(), "a.test").is_err());
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
    fn both_or_neither_path_is_accepted_and_one_alone_refused() {
        assert_eq!(paths(None, None), Ok(None));
        assert_eq!(paths(Some("c".into()), Some("k".into())), Ok(Some(("c".into(), "k".into()))));
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
