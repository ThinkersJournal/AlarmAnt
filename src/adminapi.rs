//! The loopback admin endpoint and its client (PM ruling 2026-11-13, ROADMAP.md): `alarmant enroll`
//! and `alarmant revoke` run as a thin client of the service, which is the only writer of its state.
//!
//! Server side: `POST /admin/enroll` and `POST /admin/revoke` with `Authorization: Bearer <admin
//! key>` and `{"contact":"first"|"second"}`. The key is checked first, in constant time, before the
//! body is read, so a wrong or missing key changes nothing and learns nothing; the answer is a fixed
//! 401. Operations run one at a time under one lock, so a revoke is atomic against another admin
//! command (and against a raise through [`crate::raise::raise_authorised`]'s recheck). The routes
//! belong on a **separate** server bound with [`bind_loopback`], never on the public one.
//!
//! The admin key is read from an environment variable or a file ([`admin_key_from_env`],
//! [`admin_key_from_file`]), never argv; only its digest is kept; no error text carries it. A file
//! key should sit in a directory readable by the service account only.
//!
//! Client side: [`send`] refuses any base URL that is not a loopback address, so the key cannot be
//! sent off the machine by a typo.

use std::net::SocketAddr;
use std::path::Path;
use std::sync::{Arc, Mutex};

use httpant_client::Client;
use httpant_server::core::http::header::{CACHE_CONTROL, HeaderValue};
use httpant_server::core::{Request, Response};
use httpant_server::{RunningServer, Server};

use crate::ack::{AckToken, AlertStore, bearer_token};
use crate::admin::{AdminError, Command, Contact, run};
use crate::enroll::EnrollmentStore;
use crate::register::DeviceStore;
use crate::routes::{Clock, KeyError, api_key_from_value, authorization, json_response};

/// Largest admin request body: `{"contact":"second"}` with room to spare.
pub const MAX_ADMIN_BODY_BYTES: usize = 1024;

/// What the admin routes need.
pub struct AdminConfig {
    pub admin_key: AckToken,
    pub enrollments: Arc<dyn EnrollmentStore>,
    pub devices: Arc<dyn DeviceStore>,
    pub alerts: Arc<dyn AlertStore>,
    pub clock: Clock,
}

/// Reads the admin key from the environment variable `var`.
pub fn admin_key_from_env(var: &str) -> Result<AckToken, KeyError> {
    api_key_from_value(std::env::var(var).ok().as_deref())
}

/// Reads the admin key from the first line of `path`. Unreadable and missing are the same answer.
pub fn admin_key_from_file(path: &Path) -> Result<AckToken, KeyError> {
    let text = std::fs::read_to_string(path).map_err(|_| KeyError::NotSet)?;
    api_key_from_value(text.lines().next())
}

fn parse_contact(text: &str) -> Option<Contact> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    match v.get("contact")?.as_str()? {
        "first" => Some(Contact::First),
        "second" => Some(Contact::Second),
        _ => None,
    }
}

fn no_store(mut res: Response) -> Response {
    res.headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    res
}

fn admin_post(
    server: Server,
    path: &'static str,
    make: fn(Contact) -> Command,
    cfg: &Arc<(AdminConfig, Mutex<()>)>,
) -> Server {
    let cfg = Arc::clone(cfg);
    server.post(path, move |req: Request| {
        let cfg = Arc::clone(&cfg);
        async move {
            let authorised = authorization(&req)
                .and_then(bearer_token)
                .is_some_and(|k| cfg.0.admin_key.matches(k));
            if !authorised {
                return no_store(json_response(
                    401,
                    &serde_json::json!({"error": "unauthorized"}),
                ));
            }
            let Ok(bytes) = req.into_body().bytes(MAX_ADMIN_BODY_BYTES).await else {
                return no_store(json_response(
                    413,
                    &serde_json::json!({"error": "body too large"}),
                ));
            };
            let Some(contact) = std::str::from_utf8(&bytes).ok().and_then(parse_contact) else {
                return no_store(json_response(
                    400,
                    &serde_json::json!({"error": "contact must be first or second"}),
                ));
            };
            let _one_at_a_time = cfg.1.lock().unwrap_or_else(|p| p.into_inner());
            let c = &cfg.0;
            let result = run(
                make(contact),
                &*c.enrollments,
                &*c.devices,
                &*c.alerts,
                (c.clock)(),
            );
            no_store(match result {
                Ok(text) => json_response(200, &serde_json::json!({ "text": text })),
                Err(e) => json_response(400, &serde_json::json!({ "error": e.message() })),
            })
        }
    })
}

/// Adds `POST /admin/enroll` and `POST /admin/revoke`. Put them on a server of their own and bind
/// it with [`bind_loopback`].
pub fn admin_routes(server: Server, cfg: AdminConfig) -> Server {
    let cfg = Arc::new((cfg, Mutex::new(())));
    let server = admin_post(server, "/admin/enroll", Command::Enroll, &cfg);
    admin_post(server, "/admin/revoke", Command::Revoke, &cfg)
}

/// Why the admin server was not started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindError {
    /// The address is not a loopback address: the admin endpoint is never reachable off the box.
    NotLoopback,
    /// The address could not be bound.
    Bind,
}

/// Binds `server` to `addr`, which must be a loopback address.
pub async fn bind_loopback(server: Server, addr: SocketAddr) -> Result<RunningServer, BindError> {
    if !addr.ip().is_loopback() {
        return Err(BindError::NotLoopback);
    }
    server
        .bind(addr.to_string().as_str())
        .await
        .map_err(|_| BindError::Bind)
}

fn is_loopback_base(base: &str) -> bool {
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

/// Sends `command` to the service at `base` (for example `http://127.0.0.1:9100`) and returns the
/// text to show the operator. `key` is the admin key; it goes in the `Authorization` header only.
pub async fn send(base: &str, key: &str, command: Command) -> Result<String, AdminError> {
    if !is_loopback_base(base) {
        return Err(AdminError::Remote(
            "the admin address must be http on a loopback host",
        ));
    }
    let (verb, contact) = match command {
        Command::Enroll(c) => ("enroll", c),
        Command::Revoke(c) => ("revoke", c),
    };
    let who = match contact {
        Contact::First => "first",
        Contact::Second => "second",
    };
    let res = Client::new()
        .post(format!("{}/admin/{verb}", base.trim_end_matches('/')))
        .header("authorization", format!("Bearer {key}"))
        .body(format!(r#"{{"contact":"{who}"}}"#))
        .send()
        .await
        .map_err(|_| AdminError::Remote("the service could not be reached"))?;
    let status = res.status().as_u16();
    let text = res
        .text()
        .await
        .map_err(|_| AdminError::Remote("the service answered with something unreadable"))?;
    match status {
        200 => serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| v.get("text").and_then(|t| t.as_str().map(str::to_owned)))
            .ok_or(AdminError::Remote(
                "the service answered with something unreadable",
            )),
        401 => Err(AdminError::Remote("the admin key was refused")),
        _ => Err(AdminError::Remote("the service refused the command")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ack::{AlertRecord, MemoryStore};
    use crate::enroll::MemoryEnrollments;
    use crate::fcm::DeviceToken;
    use crate::raise::{Device, DeviceRegistry};
    use crate::register::MemoryDevices;

    const KEY: &str = "0123456789abcdef0123456789abcdef0123";

    struct Rig {
        srv: RunningServer,
        enr: Arc<MemoryEnrollments>,
        dev: Arc<MemoryDevices>,
        alerts: Arc<MemoryStore>,
    }

    async fn rig() -> Rig {
        let enr = Arc::new(MemoryEnrollments::default());
        let dev = Arc::new(MemoryDevices::default());
        let alerts = Arc::new(MemoryStore::default());
        dev.set(Device {
            push_token: DeviceToken::new("fcm"),
            ack_token: AckToken::new("ack"),
        });
        alerts.insert(AlertRecord::sent("a-1", 1, AckToken::new("ack")));
        let cfg = AdminConfig {
            admin_key: AckToken::new(KEY),
            enrollments: enr.clone(),
            devices: dev.clone(),
            alerts: alerts.clone(),
            clock: Arc::new(|| 1000),
        };
        let srv = bind_loopback(
            admin_routes(Server::new(), cfg),
            "127.0.0.1:0".parse().unwrap(),
        )
        .await
        .unwrap();
        Rig {
            srv,
            enr,
            dev,
            alerts,
        }
    }

    fn base(r: &Rig) -> String {
        format!("http://{}", r.srv.addr())
    }

    async fn raw(r: &Rig, path: &str, auth: Option<&str>, body: &str) -> (u16, String) {
        let mut req = Client::new()
            .post(format!("{}{path}", base(r)))
            .body(body.to_owned());
        if let Some(a) = auth {
            req = req.header("authorization", a);
        }
        let res = req.send().await.unwrap();
        (res.status().as_u16(), res.text().await.unwrap())
    }

    #[tokio::test]
    async fn a_wrong_or_missing_key_mutates_nothing() {
        let r = rig().await;
        for auth in [
            None,
            Some("Bearer wrong"),
            Some("Bearer "),
            Some("Basic abc"),
            Some(KEY),
        ] {
            for path in ["/admin/enroll", "/admin/revoke"] {
                let (s, body) = raw(&r, path, auth, r#"{"contact":"first"}"#).await;
                assert_eq!(
                    (s, body.as_str()),
                    (401, r#"{"error":"unauthorized"}"#),
                    "{auth:?} {path}"
                );
            }
        }
        assert!(r.dev.device().is_some(), "nothing was revoked");
        assert!(r.alerts.get("a-1").unwrap().ack_token.matches("ack"));
        use crate::enroll::EnrollmentStore;
        assert!(r.enr.consume("x", 1).is_err());
        // A wrong key is refused before the body is read: a bad body still answers 401.
        let (s, _) = raw(&r, "/admin/revoke", Some("Bearer wrong"), "not json").await;
        assert_eq!(s, 401);
    }

    #[tokio::test]
    async fn enroll_and_revoke_work_with_the_key_and_the_client() {
        let r = rig().await;
        let out = send(&base(&r), KEY, Command::Enroll(Contact::Second))
            .await
            .unwrap();
        let code = out.lines().find(|l| l.len() == 32).unwrap();
        use crate::enroll::EnrollmentStore;
        assert_eq!(r.enr.consume(code, 1001), Ok(Contact::Second));
        let out = send(&base(&r), KEY, Command::Revoke(Contact::First))
            .await
            .unwrap();
        assert!(out.contains("1 alert(s)"));
        assert!(r.dev.device().is_none());
        assert!(!r.alerts.get("a-1").unwrap().ack_token.matches("ack"));
    }

    #[tokio::test]
    async fn bad_bodies_are_400_and_change_nothing() {
        let r = rig().await;
        for body in [
            "",
            "not json",
            r#"{"contact":"third"}"#,
            r#"{"contact":1}"#,
            "{}",
        ] {
            let (s, _) = raw(&r, "/admin/enroll", Some(&format!("Bearer {KEY}")), body).await;
            assert_eq!(s, 400, "{body:?}");
        }
        assert!(r.dev.device().is_some());
    }

    #[tokio::test]
    async fn the_client_reports_a_refused_key_without_echoing_it() {
        let r = rig().await;
        let e = send(
            &base(&r),
            "the-wrong-key-the-wrong-key-the-wrong",
            Command::Revoke(Contact::First),
        )
        .await
        .unwrap_err();
        assert_eq!(e, AdminError::Remote("the admin key was refused"));
        assert!(!format!("{e:?}").contains("wrong-key"));
        assert!(r.dev.device().is_some());
    }

    #[tokio::test]
    async fn the_server_binds_only_to_loopback_and_the_client_sends_only_to_loopback() {
        for bad in ["0.0.0.0:0", "192.168.1.5:9100", "[::]:0"] {
            let srv = Server::new();
            assert_eq!(
                bind_loopback(srv, bad.parse().unwrap()).await.err(),
                Some(BindError::NotLoopback),
                "{bad}"
            );
        }
        for bad in [
            "https://127.0.0.1:9100",
            "http://example.com",
            "http://192.168.1.5:9100",
            "http://127.0.0.1.evil.com",
            "127.0.0.1:9100",
        ] {
            assert!(
                send(bad, KEY, Command::Revoke(Contact::First))
                    .await
                    .is_err(),
                "{bad}"
            );
        }
        for good in ["http://127.0.0.1:1", "http://localhost:1", "http://[::1]:1"] {
            // Loopback is accepted; nothing listens, so it fails to connect, not to validate.
            assert_eq!(
                send(good, KEY, Command::Revoke(Contact::First)).await,
                Err(AdminError::Remote("the service could not be reached")),
                "{good}"
            );
        }
    }

    #[test]
    fn the_key_file_is_read_trimmed_and_short_or_missing_ones_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("admin.key");
        std::fs::write(&p, format!("  {KEY}  \nsecond line\n")).unwrap();
        assert!(admin_key_from_file(&p).unwrap().matches(KEY));
        std::fs::write(&p, "short").unwrap();
        assert_eq!(admin_key_from_file(&p).err(), Some(KeyError::TooShort));
        assert_eq!(
            admin_key_from_file(&dir.path().join("nope")).err(),
            Some(KeyError::NotSet)
        );
    }
}
