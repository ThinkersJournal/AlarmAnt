//! The HTTP routes, on HTTPAnt's server half. Each route is a small function that adds itself to a
//! [`Server`]; the handlers hold no logic of their own, they pass the header, the path and the clock
//! to the framework-free handlers ([`crate::ack`], later [`crate::raise`]).

use std::fmt;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use httpant_server::Server;
use httpant_server::core::http::StatusCode;
use httpant_server::core::http::header::{AUTHORIZATION, CONTENT_TYPE, HeaderValue};
use httpant_server::core::{Body, IntoResponse, PathParams, Request, Response};

use crate::ack::{AckToken, AlertStore, acknowledge};
use crate::fcm::PushTransport;
use crate::raise::{DeviceRegistry, raise_alert, response};

/// Largest request body read for `POST /alert`: the title and body limits at four bytes a
/// character, plus JSON framing, rounded up.
pub const MAX_RAISE_BODY_BYTES: usize = 16 * 1024;

/// The shortest accepted API key, in characters (`openssl rand -hex 32` gives 64).
pub const MIN_API_KEY_CHARS: usize = 32;

/// Why the API key could not be loaded. The text never includes the value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyError {
    /// The environment variable is not set (or is not UTF-8).
    NotSet,
    /// Shorter than [`MIN_API_KEY_CHARS`] once surrounding whitespace is removed.
    TooShort,
}

impl fmt::Display for KeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyError::NotSet => f.write_str("the API key environment variable is not set"),
            KeyError::TooShort => write!(
                f,
                "the API key is shorter than {MIN_API_KEY_CHARS} characters"
            ),
        }
    }
}

impl std::error::Error for KeyError {}

/// Checks a key value read from configuration: trims whitespace, refuses a short one.
pub fn api_key_from_value(value: Option<&str>) -> Result<AckToken, KeyError> {
    let v = value.ok_or(KeyError::NotSet)?.trim();
    if v.chars().count() < MIN_API_KEY_CHARS {
        return Err(KeyError::TooShort);
    }
    Ok(AckToken::new(v))
}

/// Reads the API key for `POST /alert` from the environment variable `var` (a secret: environment
/// only, never a file in the repository).
pub fn api_key_from_env(var: &str) -> Result<AckToken, KeyError> {
    api_key_from_value(std::env::var(var).ok().as_deref())
}

/// Where "now" comes from, in unix seconds. Tests pass a fixed one.
pub type Clock = Arc<dyn Fn() -> u64 + Send + Sync>;

/// The system clock.
pub fn system_clock() -> Clock {
    Arc::new(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    })
}

/// The `Authorization` header as text, if present and visible ASCII.
fn authorization(req: &Request) -> Option<&str> {
    req.headers().get(AUTHORIZATION)?.to_str().ok()
}

/// Adds `POST /ack/:alert_id` (A4). Answers 200 for a first or repeated acknowledgement, 401 for a
/// bad or missing token, 404 for an unknown alert; the body is empty and never repeats the request.
pub fn ack_route(server: Server, store: Arc<dyn AlertStore>, clock: Clock) -> Server {
    server.post("/ack/:alert_id", move |req: Request| {
        let (store, clock) = (Arc::clone(&store), Arc::clone(&clock));
        async move {
            let id = req
                .extensions()
                .get::<PathParams>()
                .and_then(|p| p.get("alert_id"))
                .map(str::to_owned);
            let Some(id) = id else {
                return StatusCode::NOT_FOUND.into_response();
            };
            let outcome = acknowledge(&*store, &id, authorization(&req), clock());
            let status = StatusCode::from_u16(outcome.status()).unwrap_or(StatusCode::NOT_FOUND);
            let res: Response = status.into_response();
            res
        }
    })
}

/// What `POST /alert` needs. One value, cloned cheaply into the handler.
pub struct RaiseConfig<T> {
    pub api_key: AckToken,
    pub devices: Arc<dyn DeviceRegistry>,
    pub transport: Arc<T>,
    pub store: Arc<dyn AlertStore>,
    pub clock: Clock,
}

fn json_response(status: u16, body: &serde_json::Value) -> Response {
    let mut res = Response::new(Body::from_bytes(body.to_string()));
    *res.status_mut() = StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    res.headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    res
}

/// Adds `POST /alert` (A9): `Authorization: Bearer <api key>`, JSON `{"title","body"}`; answers 202
/// with `{"alert_id"}` or the status in [`crate::raise::RaiseError::status`]. A body over
/// [`MAX_RAISE_BODY_BYTES`] is 413; a body that is not UTF-8 is 400.
pub fn raise_route<T>(server: Server, cfg: RaiseConfig<T>) -> Server
where
    T: PushTransport + Send + Sync + 'static,
{
    let api_key = Arc::new(cfg.api_key);
    let (devices, transport, store, clock) = (cfg.devices, cfg.transport, cfg.store, cfg.clock);
    server.post("/alert", move |req: Request| {
        let (api_key, devices, transport, store, clock) = (
            Arc::clone(&api_key),
            Arc::clone(&devices),
            Arc::clone(&transport),
            Arc::clone(&store),
            Arc::clone(&clock),
        );
        async move {
            let auth = authorization(&req).map(str::to_owned);
            let bytes = match req.into_body().bytes(MAX_RAISE_BODY_BYTES).await {
                Ok(b) => b,
                Err(_) => {
                    return json_response(413, &serde_json::json!({"error": "body too large"}));
                }
            };
            let Ok(text) = std::str::from_utf8(&bytes) else {
                return json_response(400, &serde_json::json!({"error": "body is not UTF-8"}));
            };
            let result = raise_alert(
                &api_key,
                auth.as_deref(),
                text,
                &*devices,
                &*transport,
                &*store,
                clock(),
            )
            .await;
            let (status, body) = response(&result);
            json_response(status, &body)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AlertState;
    use crate::ack::{AckToken, AlertRecord, MemoryStore};
    use httpant_client::Client;

    async fn serve(store: Arc<MemoryStore>) -> httpant_server::RunningServer {
        let s = store as Arc<dyn AlertStore>;
        ack_route(Server::new(), s, Arc::new(|| 777))
            .bind("127.0.0.1:0")
            .await
            .unwrap()
    }

    async fn post(srv: &httpant_server::RunningServer, path: &str, auth: Option<&str>) -> u16 {
        let mut r = Client::new().post(format!("http://{}{path}", srv.addr()));
        if let Some(a) = auth {
            r = r.header("authorization", a);
        }
        r.send().await.unwrap().status().as_u16()
    }

    fn store() -> Arc<MemoryStore> {
        let s = Arc::new(MemoryStore::default());
        s.insert(AlertRecord::sent("a-1", 100, AckToken::new("s3cret")));
        s
    }

    #[tokio::test]
    async fn first_ack_is_200_then_repeat_is_200_and_keeps_the_first_time() {
        let s = store();
        let srv = serve(Arc::clone(&s)).await;
        assert_eq!(post(&srv, "/ack/a-1", Some("Bearer s3cret")).await, 200);
        assert_eq!(s.get("a-1").unwrap().state, AlertState::Acked);
        assert_eq!(s.get("a-1").unwrap().acked_at, Some(777));
        assert_eq!(post(&srv, "/ack/a-1", Some("Bearer s3cret")).await, 200);
        srv.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn bad_token_is_401_and_unknown_alert_is_404() {
        let s = store();
        let srv = serve(Arc::clone(&s)).await;
        for auth in [
            None,
            Some("Bearer nope"),
            Some("s3cret"),
            Some("Basic s3cret"),
        ] {
            assert_eq!(post(&srv, "/ack/a-1", auth).await, 401, "{auth:?}");
        }
        assert_eq!(s.get("a-1").unwrap().state, AlertState::Sent);
        assert_eq!(post(&srv, "/ack/zzz", Some("Bearer s3cret")).await, 404);
        srv.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn only_post_is_routed() {
        let srv = serve(store()).await;
        let r = Client::new()
            .get(format!("http://{}/ack/a-1", srv.addr()))
            .header("authorization", "Bearer s3cret")
            .send()
            .await
            .unwrap();
        assert!(matches!(r.status().as_u16(), 404 | 405), "{}", r.status());
        srv.shutdown().await.unwrap();
    }

    // ---- A9 ----
    use crate::fcm::{AlertMessage, DeviceToken, PushError, PushReceipt};
    use crate::raise::Device;
    use std::future::Future;
    use std::sync::Mutex;

    const KEY: &str = "0123456789abcdef0123456789abcdef";

    #[derive(Default)]
    struct Fake {
        fail: Option<PushError>,
        sent: Mutex<Vec<AlertMessage>>,
    }

    impl PushTransport for Fake {
        fn send(
            &self,
            _: &DeviceToken,
            alert: &AlertMessage,
        ) -> impl Future<Output = Result<PushReceipt, PushError>> + Send {
            let r = match &self.fail {
                Some(e) => Err(e.clone()),
                None => {
                    self.sent.lock().unwrap().push(alert.clone());
                    Ok(PushReceipt {
                        message_name: "m".into(),
                    })
                }
            };
            async move { r }
        }
    }

    struct One(Option<Device>);

    impl DeviceRegistry for One {
        fn device(&self) -> Option<Device> {
            self.0.clone()
        }
    }

    fn dev() -> Option<Device> {
        Some(Device {
            push_token: DeviceToken::new("fcm"),
            ack_token: AckToken::new("ack-tok"),
        })
    }

    async fn serve_raise(
        fake: Arc<Fake>,
        device: Option<Device>,
        store: Arc<MemoryStore>,
    ) -> httpant_server::RunningServer {
        raise_route(
            Server::new(),
            RaiseConfig {
                api_key: AckToken::new(KEY),
                devices: Arc::new(One(device)),
                transport: fake,
                store: store as Arc<dyn AlertStore>,
                clock: Arc::new(|| 50),
            },
        )
        .bind("127.0.0.1:0")
        .await
        .unwrap()
    }

    async fn raise(
        srv: &httpant_server::RunningServer,
        auth: Option<&str>,
        body: impl Into<httpant_server::core::Bytes>,
    ) -> (u16, String, Option<String>) {
        let mut r = Client::new()
            .post(format!("http://{}/alert", srv.addr()))
            .body(body);
        if let Some(a) = auth {
            r = r.header("authorization", a);
        }
        let res = r.send().await.unwrap();
        let status = res.status().as_u16();
        let ct = res
            .headers()
            .get("content-type")
            .map(|v| v.to_str().unwrap().to_owned());
        (status, res.text().await.unwrap(), ct)
    }

    const OK_BODY: &str = r#"{"title":"DB down","body":"prod-1"}"#;

    #[tokio::test]
    async fn raise_over_http_is_202_with_an_id_the_phone_can_then_ack() {
        let (fake, store) = (Arc::new(Fake::default()), Arc::new(MemoryStore::default()));
        let srv = serve_raise(Arc::clone(&fake), dev(), Arc::clone(&store)).await;
        let auth = format!("Bearer {KEY}");
        let (status, text, ct) = raise(&srv, Some(&auth), OK_BODY).await;
        assert_eq!(status, 202);
        assert_eq!(ct.as_deref(), Some("application/json"));
        let id = serde_json::from_str::<serde_json::Value>(&text).unwrap()["alert_id"]
            .as_str()
            .unwrap()
            .to_owned();
        assert_eq!(fake.sent.lock().unwrap()[0].title, "DB down");
        assert_eq!(store.get(&id).unwrap().sent_at, 50);
        let ack = acknowledge(&*store, &id, Some("Bearer ack-tok"), 60);
        assert_eq!(ack, crate::ack::AckOutcome::Acked);
        srv.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn raise_failures_have_the_documented_statuses() {
        let auth = format!("Bearer {KEY}");
        // bad key, bad body
        let (fake, store) = (Arc::new(Fake::default()), Arc::new(MemoryStore::default()));
        let srv = serve_raise(Arc::clone(&fake), dev(), store).await;
        for a in [None, Some("Bearer nope"), Some(KEY)] {
            assert_eq!(raise(&srv, a, OK_BODY).await.0, 401, "{a:?}");
        }
        assert_eq!(raise(&srv, Some(&auth), "not json").await.0, 400);
        assert_eq!(raise(&srv, Some(&auth), vec![0xff, 0xfe]).await.0, 400);
        assert!(fake.sent.lock().unwrap().is_empty());
        srv.shutdown().await.unwrap();
        // too large
        let srv = serve_raise(Arc::default(), dev(), Arc::default()).await;
        let big = vec![b' '; MAX_RAISE_BODY_BYTES + 1];
        assert_eq!(raise(&srv, Some(&auth), big).await.0, 413);
        srv.shutdown().await.unwrap();
        // no device
        let srv = serve_raise(Arc::default(), None, Arc::default()).await;
        assert_eq!(raise(&srv, Some(&auth), OK_BODY).await.0, 409);
        srv.shutdown().await.unwrap();
        // push failures
        for (fail, want) in [
            (PushError::Transient("x".into()), 503),
            (PushError::TokenGone, 410),
            (PushError::Permanent("x".into()), 502),
        ] {
            let fake = Arc::new(Fake {
                fail: Some(fail),
                ..Fake::default()
            });
            let store = Arc::new(MemoryStore::default());
            let srv = serve_raise(fake, dev(), Arc::clone(&store)).await;
            assert_eq!(raise(&srv, Some(&auth), OK_BODY).await.0, want);
            assert!(store.is_empty());
            srv.shutdown().await.unwrap();
        }
    }

    #[test]
    fn api_key_rules() {
        assert_eq!(api_key_from_value(None).unwrap_err(), KeyError::NotSet);
        assert_eq!(
            api_key_from_value(Some("short")).unwrap_err(),
            KeyError::TooShort
        );
        assert_eq!(
            api_key_from_value(Some("   ")).unwrap_err(),
            KeyError::TooShort
        );
        let padded = format!(
            "  {KEY}
"
        );
        let k = api_key_from_value(Some(&padded)).unwrap();
        assert!(k.matches(KEY));
        assert!(!format!("{k:?}").contains(KEY));
        assert!(!KeyError::TooShort.to_string().contains("short\""));
        assert_eq!(
            api_key_from_env("ALARMANT_TEST_KEY_THAT_IS_NOT_SET").unwrap_err(),
            KeyError::NotSet
        );
    }
}
