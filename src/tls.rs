//! TLS for the public listener: a certificate and key the operator supplies (`ALARMANT_TLS_CERT_FILE`
//! and `ALARMANT_TLS_KEY_FILE`, both or neither). Without them the server speaks plain HTTP and is meant
//! to sit behind a TLS proxy; a phone only enrols against an `https://` address.
//!
//! Errors are fixed text: they never carry the key, a path's contents or a library message.

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
