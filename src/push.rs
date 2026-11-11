//! A2 slice 2b: the real [`PushTransport`], over HTTPAnt.
//!
//! `send` signs in with the service account (cached by [`TokenCache`], refreshed a minute early),
//! posts the alert to FCM HTTP v1 and classifies the answer: 2xx accepted; `UNREGISTERED` is
//! [`PushError::TokenGone`]; 429 and 5xx and network failures are transient; a 401 clears the
//! cached token and retries once; anything else is permanent. Every error string is fixed text or
//! a short provider code: never a token, the key, the device token or a response body.
//!
//! The FCM and token endpoints are configurable so the tests run against a local fake server; no
//! real FCM call is made anywhere in this crate's tests (the key is board 354).

use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use httpant_client::Client;
use serde_json::Value;

use crate::auth::{
    AuthError, ServiceAccount, TokenCache, parse_token_response, token_request_body,
};
use crate::fcm::{
    AlertMessage, DeviceToken, FCM_BASE, PushError, PushReceipt, PushTransport, request_body,
    send_url_at,
};

/// Longest response body read from either endpoint.
const MAX_BODY: usize = 64 * 1024;

type Clock = Arc<dyn Fn() -> u64 + Send + Sync>;

/// FCM HTTP v1 over HTTPAnt.
///
/// The FCM base URL cannot be changed outside test builds:
///
/// ```compile_fail
/// fn f(t: alarmant::push::HttpPushTransport) {
/// let _ = t.with_fcm_base("http://example.invalid");
/// }
/// ```
pub struct HttpPushTransport {
    client: Client,
    account: ServiceAccount,
    fcm_base: String,
    cache: Mutex<TokenCache>,
    clock: Clock,
}

impl fmt::Debug for HttpPushTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpPushTransport")
            .field("account", &self.account)
            .field("fcm_base", &self.fcm_base)
            .finish_non_exhaustive()
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl HttpPushTransport {
    /// A transport with HTTPAnt's default adapter and 5 s connect / 15 s total timeouts.
    pub fn new(account: ServiceAccount) -> Result<Self, PushError> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|_| PushError::Permanent("http client could not be built".into()))?;
        Ok(Self::with_client(account, client))
    }

    /// A transport on a client you built (another adapter, other timeouts).
    pub fn with_client(account: ServiceAccount, client: Client) -> Self {
        HttpPushTransport {
            client,
            account,
            fcm_base: FCM_BASE.to_string(),
            cache: Mutex::new(TokenCache::default()),
            clock: Arc::new(unix_now),
        }
    }

    /// Point the FCM call at another base URL (a local fake server). Test builds only, like
    /// `ServiceAccount::with_token_uri`: the shipped transport can only talk to the real endpoint.
    #[cfg(test)]
    pub(crate) fn with_fcm_base(mut self, base: impl Into<String>) -> Self {
        self.fcm_base = base.into();
        self
    }

    /// Replace the clock (unix seconds), for tests of token refresh.
    pub fn with_clock(mut self, clock: impl Fn() -> u64 + Send + Sync + 'static) -> Self {
        self.clock = Arc::new(clock);
        self
    }

    fn cache(&self) -> std::sync::MutexGuard<'_, TokenCache> {
        self.cache.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A valid access token: the cached one, or a fresh one from the token endpoint.
    async fn access_token(&self) -> Result<String, PushError> {
        let now = (self.clock)();
        if let Some(t) = self.cache().valid(now) {
            return Ok(t.expose().to_owned());
        }
        let assertion = self
            .account
            .assertion(now)
            .map_err(|_| PushError::Permanent("service-account key unusable".into()))?;
        let resp = self
            .client
            .post(self.account.token_uri())
            .header("content-type", "application/x-www-form-urlencoded")
            .body(token_request_body(&assertion))
            .send()
            .await
            .map_err(|_| PushError::Transient("token endpoint unreachable".into()))?;
        let status = resp.status().as_u16();
        let text = resp
            .text_limited(MAX_BODY)
            .await
            .map_err(|_| PushError::Transient("token endpoint answer unreadable".into()))?;
        if status == 429 || status >= 500 {
            return Err(PushError::Transient(format!("token endpoint {status}")));
        }
        match parse_token_response(&text, now) {
            Ok(token) => {
                let value = token.expose().to_owned();
                self.cache().store(token);
                Ok(value)
            }
            Err(AuthError::Rejected(code)) => {
                Err(PushError::Permanent(format!("sign-in rejected: {code}")))
            }
            Err(_) => Err(PushError::Permanent("sign-in failed".into())),
        }
    }

    async fn send_once(
        &self,
        token: &DeviceToken,
        alert: &AlertMessage,
        access: &str,
    ) -> Result<Attempt, PushError> {
        let url = send_url_at(&self.fcm_base, self.account.project_id());
        let resp = self
            .client
            .post(url)
            .bearer(access)
            .header("content-type", "application/json")
            .body(request_body(token, alert).to_string())
            .send()
            .await
            .map_err(|_| PushError::Transient("push service unreachable".into()))?;
        let status = resp.status().as_u16();
        let text = resp
            .text_limited(MAX_BODY)
            .await
            .map_err(|_| PushError::Transient("push service answer unreadable".into()))?;
        classify(status, &text)
    }
}

/// What one FCM call came to.
enum Attempt {
    Accepted(PushReceipt),
    /// 401: the access token was refused; sign in again.
    AccessTokenRefused,
}

/// Maps an FCM answer to a result. Pure, so it is tested without a server.
fn classify(status: u16, body: &str) -> Result<Attempt, PushError> {
    let json: Option<Value> = serde_json::from_str(body).ok();
    match status {
        200..=299 => {
            let name = json
                .as_ref()
                .and_then(|v| v.get("name"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            Ok(Attempt::Accepted(PushReceipt {
                message_name: name.chars().take(200).collect(),
            }))
        }
        401 => Ok(Attempt::AccessTokenRefused),
        404 if fcm_error_code(json.as_ref()) == Some("UNREGISTERED") => Err(PushError::TokenGone),
        429 | 500..=599 => Err(PushError::Transient(format!("push service {status}"))),
        _ => {
            let code = fcm_error_code(json.as_ref())
                .or_else(|| {
                    json.as_ref()
                        .and_then(|v| v.pointer("/error/status"))
                        .and_then(Value::as_str)
                })
                .map(|c| c.chars().take(40).collect::<String>());
            Err(PushError::Permanent(match code {
                Some(c) => format!("push service {status} {c}"),
                None => format!("push service {status}"),
            }))
        }
    }
}

/// FCM's `error.details[].errorCode`, if the answer has one.
fn fcm_error_code(json: Option<&Value>) -> Option<&str> {
    json?
        .pointer("/error/details")?
        .as_array()?
        .iter()
        .find_map(|d| d.get("errorCode").and_then(Value::as_str))
}

impl PushTransport for HttpPushTransport {
    async fn send(
        &self,
        token: &DeviceToken,
        alert: &AlertMessage,
    ) -> Result<PushReceipt, PushError> {
        // At most two attempts: a 401 means the cached token was revoked early.
        for attempt in 0..2 {
            let access = self.access_token().await?;
            match self.send_once(token, alert, &access).await? {
                Attempt::Accepted(receipt) => return Ok(receipt),
                Attempt::AccessTokenRefused => {
                    self.cache().clear();
                    if attempt == 1 {
                        break;
                    }
                }
            }
        }
        Err(PushError::Permanent(
            "push service refused the sign-in".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::tests::account_json_for;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::{SocketAddr, TcpListener};

    #[derive(Debug, Clone)]
    struct Seen {
        request_line: String,
        headers: Vec<(String, String)>,
        body: String,
    }

    impl Seen {
        fn header(&self, name: &str) -> Option<&str> {
            self.headers
                .iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.as_str())
        }
    }

    struct Fake {
        addr: SocketAddr,
        seen: Arc<Mutex<Vec<Seen>>>,
    }

    impl Fake {
        fn url(&self) -> String {
            format!("http://{}", self.addr)
        }
        fn seen(&self) -> Vec<Seen> {
            self.seen.lock().unwrap().clone()
        }
    }

    /// A one-thread HTTP/1.1 server: connection n gets scripted answer n, then it closes.
    fn fake(script: Vec<(u16, String)>) -> Fake {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        std::thread::spawn(move || {
            for (status, body) in script {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut request_line = String::new();
                reader.read_line(&mut request_line).unwrap();
                let mut headers = Vec::new();
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    let line = line.trim_end().to_string();
                    if line.is_empty() {
                        break;
                    }
                    if let Some((k, v)) = line.split_once(':') {
                        headers.push((k.trim().to_ascii_lowercase(), v.trim().to_string()));
                    }
                }
                let len = headers
                    .iter()
                    .find(|(k, _)| k == "content-length")
                    .and_then(|(_, v)| v.parse::<usize>().ok())
                    .unwrap_or(0);
                let mut raw = vec![0u8; len];
                reader.read_exact(&mut raw).unwrap();
                log.lock().unwrap().push(Seen {
                    request_line: request_line.trim_end().to_string(),
                    headers,
                    body: String::from_utf8_lossy(&raw).into_owned(),
                });
                let reply = format!(
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(reply.as_bytes());
            }
        });
        Fake { addr, seen }
    }

    const TOKEN_OK: &str =
        r#"{"access_token":"ya29.SECRET-ACCESS","expires_in":3600,"token_type":"Bearer"}"#;
    const FCM_OK: &str = r#"{"name":"projects/alarmant-test/messages/0:123"}"#;
    const DEVICE: &str = "device-token-XYZ";

    fn ok(body: &str) -> (u16, String) {
        (200, body.to_string())
    }

    fn account_at(token_uri: String) -> ServiceAccount {
        ServiceAccount::from_json(&account_json_for("https://oauth2.googleapis.com/token"))
            .unwrap()
            .with_token_uri(token_uri)
    }

    fn transport(server: &Fake) -> HttpPushTransport {
        HttpPushTransport::new(account_at(format!("{}/token", server.url())))
            .unwrap()
            .with_fcm_base(server.url())
            .with_clock(|| 1_000_000)
    }

    fn alert() -> AlertMessage {
        AlertMessage {
            alert_id: "a-1".into(),
            title: "DB down".into(),
            body: "prod-1".into(),
        }
    }

    async fn send(t: &HttpPushTransport) -> Result<PushReceipt, PushError> {
        t.send(&DeviceToken::new(DEVICE), &alert()).await
    }

    fn all_text(r: &Result<PushReceipt, PushError>) -> String {
        format!("{r:?}")
    }

    #[tokio::test(flavor = "current_thread")]
    async fn signs_in_then_posts_the_alert_with_the_bearer_token() {
        let server = fake(vec![ok(TOKEN_OK), ok(FCM_OK)]);
        let t = transport(&server);
        let receipt = send(&t).await.unwrap();
        assert_eq!(
            receipt.message_name,
            "projects/alarmant-test/messages/0:123"
        );
        let seen = server.seen();
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0].request_line, "POST /token HTTP/1.1");
        assert_eq!(
            seen[0].header("content-type"),
            Some("application/x-www-form-urlencoded")
        );
        assert!(seen[0].body.starts_with(
            "grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Ajwt-bearer&assertion="
        ));
        assert_eq!(
            seen[1].request_line,
            "POST /v1/projects/alarmant-test/messages:send HTTP/1.1"
        );
        assert_eq!(
            seen[1].header("authorization"),
            Some("Bearer ya29.SECRET-ACCESS")
        );
        assert_eq!(seen[1].header("content-type"), Some("application/json"));
        let sent: Value = serde_json::from_str(&seen[1].body).unwrap();
        assert_eq!(sent, request_body(&DeviceToken::new(DEVICE), &alert()));
        assert!(sent["message"].get("notification").is_none());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn the_access_token_is_cached_between_sends() {
        let server = fake(vec![ok(TOKEN_OK), ok(FCM_OK), ok(FCM_OK)]);
        let t = transport(&server);
        send(&t).await.unwrap();
        send(&t).await.unwrap();
        let paths: Vec<_> = server
            .seen()
            .iter()
            .map(|s| s.request_line.split(' ').nth(1).unwrap().to_string())
            .collect();
        assert_eq!(
            paths,
            vec![
                "/token",
                "/v1/projects/alarmant-test/messages:send",
                "/v1/projects/alarmant-test/messages:send"
            ]
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_token_near_expiry_is_replaced() {
        let short = r#"{"access_token":"ya29.SHORT","expires_in":100}"#;
        let server = fake(vec![ok(short), ok(FCM_OK), ok(TOKEN_OK), ok(FCM_OK)]);
        let now = Arc::new(Mutex::new(1_000_000u64));
        let clock = now.clone();
        let t = transport(&server).with_clock(move || *clock.lock().unwrap());
        send(&t).await.unwrap();
        *now.lock().unwrap() += 50; // 50 s left, inside the 60 s margin
        send(&t).await.unwrap();
        let seen = server.seen();
        assert_eq!(seen.len(), 4);
        assert_eq!(seen[1].header("authorization"), Some("Bearer ya29.SHORT"));
        assert_eq!(
            seen[3].header("authorization"),
            Some("Bearer ya29.SECRET-ACCESS")
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_401_clears_the_token_and_retries_once() {
        let server = fake(vec![
            ok(TOKEN_OK),
            (401, r#"{"error":{"status":"UNAUTHENTICATED"}}"#.into()),
            ok(r#"{"access_token":"ya29.SECOND","expires_in":3600}"#),
            ok(FCM_OK),
        ]);
        let t = transport(&server);
        assert!(send(&t).await.is_ok());
        let seen = server.seen();
        assert_eq!(seen.len(), 4);
        assert_eq!(seen[3].header("authorization"), Some("Bearer ya29.SECOND"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn two_401s_in_a_row_are_permanent() {
        let denied = || (401, r#"{"error":{"status":"UNAUTHENTICATED"}}"#.to_string());
        let server = fake(vec![ok(TOKEN_OK), denied(), ok(TOKEN_OK), denied()]);
        let t = transport(&server);
        assert!(matches!(send(&t).await, Err(PushError::Permanent(_))));
        assert_eq!(server.seen().len(), 4, "no third attempt");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn unregistered_is_token_gone_but_other_404s_are_not() {
        let gone = r#"{"error":{"code":404,"status":"NOT_FOUND","details":[{"@type":"x","errorCode":"UNREGISTERED"}]}}"#;
        let server = fake(vec![ok(TOKEN_OK), (404, gone.into())]);
        assert_eq!(send(&transport(&server)).await, Err(PushError::TokenGone));
        let server = fake(vec![
            ok(TOKEN_OK),
            (404, r#"{"error":{"status":"NOT_FOUND"}}"#.into()),
        ]);
        assert!(matches!(
            send(&transport(&server)).await,
            Err(PushError::Permanent(_))
        ));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn throttling_and_server_errors_are_transient_and_the_rest_permanent() {
        for (status, transient) in [
            (429, true),
            (500, true),
            (503, true),
            (400, false),
            (403, false),
        ] {
            let server = fake(vec![ok(TOKEN_OK), (status, "{}".into())]);
            let r = send(&transport(&server)).await;
            match (transient, &r) {
                (true, Err(PushError::Transient(_))) | (false, Err(PushError::Permanent(_))) => {}
                _ => panic!("{status}: {r:?}"),
            }
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn sign_in_failures_are_classified() {
        let server = fake(vec![(
            400,
            r#"{"error":"invalid_grant","error_description":"Invalid JWT: secret detail"}"#.into(),
        )]);
        let r = send(&transport(&server)).await;
        assert_eq!(
            r,
            Err(PushError::Permanent(
                "sign-in rejected: invalid_grant".into()
            ))
        );
        assert!(!all_text(&r).contains("secret detail"));
        let server = fake(vec![(503, "{}".into())]);
        assert!(matches!(
            send(&transport(&server)).await,
            Err(PushError::Transient(_))
        ));
        assert_eq!(server.seen().len(), 1, "no FCM call without a token");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn an_unreachable_server_is_transient() {
        let closed = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = closed.local_addr().unwrap();
        drop(closed);
        let account = account_at(format!("http://{addr}/token"));
        let t = HttpPushTransport::new(account)
            .unwrap()
            .with_fcm_base(format!("http://{addr}"));
        assert!(matches!(send(&t).await, Err(PushError::Transient(_))));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn errors_and_debug_never_carry_secrets_or_response_bodies() {
        let echo = format!(
            r#"{{"error":{{"status":"INVALID_ARGUMENT","message":"bad token {DEVICE} ya29.SECRET-ACCESS"}}}}"#
        );
        let server = fake(vec![ok(TOKEN_OK), (400, echo)]);
        let t = transport(&server);
        let r = send(&t).await;
        assert_eq!(
            r,
            Err(PushError::Permanent(
                "push service 400 INVALID_ARGUMENT".into()
            ))
        );
        let text = format!("{} {t:?}", all_text(&r));
        for (i, secret) in [DEVICE, "ya29.SECRET-ACCESS", "PRIVATE KEY", "MIIEv"]
            .iter()
            .enumerate()
        {
            assert!(!text.contains(secret), "fixture {i} leaked");
        }
    }

    #[test]
    fn classify_reads_the_receipt_and_tolerates_a_missing_name() {
        let Ok(Attempt::Accepted(r)) = classify(200, FCM_OK) else {
            panic!("not accepted");
        };
        assert_eq!(r.message_name, "projects/alarmant-test/messages/0:123");
        let Ok(Attempt::Accepted(r)) = classify(200, "not json") else {
            panic!("not accepted");
        };
        assert_eq!(r.message_name, "");
    }
}
