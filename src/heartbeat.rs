// SPDX-License-Identifier: MIT OR Apache-2.0
//! `POST /heartbeat` (docs/SYNAPSE-ADAPTER.md 5.1): the bridge reports every 60 seconds with its own
//! key, which can post heartbeats and nothing else. The body is the fixed shape of
//! [`crate::watchdog::parse_heartbeat`]; the answer is an empty status. Each accepted beat feeds the
//! observer A machine and whatever it asks for goes to the sink (the page path).
//!
//! The route reads nothing from the phone or the PC's polls and returns nothing about the system:
//! a stolen key can only hold a page off, never raise one or read anything.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use httpant_server::Server;
use httpant_server::core::http::StatusCode;
use httpant_server::core::{IntoResponse, Request, Response};

use crate::ack::{AckToken, bearer_token};
use crate::routes::{Clock, authorization};
use crate::watchdog::{Action, Watchdog, parse_heartbeat};

/// Largest body read: the fixed shape is about 50 bytes.
pub const MAX_HEARTBEAT_BODY_BYTES: usize = 256;
/// Most beats accepted in any rolling minute (the cadence is one a minute).
pub const MAX_BEATS_PER_MINUTE: usize = 10;

/// Where the watchdog's actions go (the page path, and the audit log).
pub type ActionSink = Arc<dyn Fn(Vec<Action>) + Send + Sync>;

/// What `POST /heartbeat` needs.
pub struct HeartbeatConfig {
    /// The heartbeat key. Distinct from the API key; checked by digest.
    pub key: AckToken,
    pub watchdog: Arc<Mutex<Watchdog>>,
    pub sink: ActionSink,
    pub clock: Clock,
}

/// Adds `POST /heartbeat`: 200 accepted, 400 not the fixed shape (or not UTF-8), 401 wrong or missing
/// key, 413 too large, 429 over the per-minute cap. Nothing is fed to the watchdog unless accepted.
pub fn heartbeat_route(server: Server, cfg: HeartbeatConfig) -> Server {
    let key = Arc::new(cfg.key);
    let recent: Arc<Mutex<VecDeque<u64>>> = Arc::default();
    let (wd, sink, clock) = (cfg.watchdog, cfg.sink, cfg.clock);
    server.post("/heartbeat", move |req: Request| {
        let (key, recent, wd, sink, clock) = (
            Arc::clone(&key),
            Arc::clone(&recent),
            Arc::clone(&wd),
            Arc::clone(&sink),
            Arc::clone(&clock),
        );
        async move {
            let ok = authorization(&req)
                .and_then(bearer_token)
                .is_some_and(|t| key.matches(t));
            if !ok {
                return StatusCode::UNAUTHORIZED.into_response();
            }
            let Ok(bytes) = req.into_body().bytes(MAX_HEARTBEAT_BODY_BYTES).await else {
                return StatusCode::PAYLOAD_TOO_LARGE.into_response();
            };
            let Some(hb) = std::str::from_utf8(&bytes).ok().and_then(parse_heartbeat) else {
                return StatusCode::BAD_REQUEST.into_response();
            };
            let now = clock();
            {
                let mut r = recent.lock().unwrap_or_else(|e| e.into_inner());
                while r.front().is_some_and(|&t| now >= t + 60) {
                    r.pop_front();
                }
                if r.len() >= MAX_BEATS_PER_MINUTE {
                    return StatusCode::TOO_MANY_REQUESTS.into_response();
                }
                r.push_back(now);
            }
            let actions = wd.lock().unwrap_or_else(|e| e.into_inner()).beat(now, hb);
            if !actions.is_empty() {
                sink(actions);
            }
            let res: Response = StatusCode::OK.into_response();
            res
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpant_client::Client;
    use std::sync::atomic::{AtomicU64, Ordering};

    const KEY: &str = "heartbeat-key-0123456789abcdef0123";
    const GOOD: &str = r#"{"synapse":"up","canary":"ok","bridge":"up"}"#;
    const DOWN: &str = r#"{"synapse":"down","canary":"ok","bridge":"up"}"#;

    struct Rig {
        srv: httpant_server::RunningServer,
        wd: Arc<Mutex<Watchdog>>,
        got: Arc<Mutex<Vec<Action>>>,
        now: Arc<AtomicU64>,
    }

    async fn rig() -> Rig {
        let now = Arc::new(AtomicU64::new(1_000));
        let wd = Arc::new(Mutex::new(Watchdog::new(1_000)));
        let got: Arc<Mutex<Vec<Action>>> = Arc::default();
        let g = Arc::clone(&got);
        let n = Arc::clone(&now);
        let srv = heartbeat_route(
            Server::new(),
            HeartbeatConfig {
                key: AckToken::new(KEY),
                watchdog: Arc::clone(&wd),
                sink: Arc::new(move |a| g.lock().unwrap().extend(a)),
                clock: Arc::new(move || n.load(Ordering::SeqCst)),
            },
        )
        .bind("127.0.0.1:0")
        .await
        .unwrap();
        Rig { srv, wd, got, now }
    }

    async fn post(r: &Rig, auth: Option<&str>, body: &str) -> u16 {
        let mut q = Client::new()
            .post(format!("http://{}/heartbeat", r.srv.addr()))
            .body(body.to_owned());
        if let Some(a) = auth {
            q = q.header("authorization", a);
        }
        q.send().await.unwrap().status().as_u16()
    }

    fn bearer() -> String {
        format!("Bearer {KEY}")
    }

    #[tokio::test]
    async fn a_good_beat_is_accepted_and_reaches_the_watchdog() {
        let r = rig().await;
        assert_eq!(post(&r, Some(&bearer()), GOOD).await, 200);
        assert!(r.got.lock().unwrap().is_empty());
        // the beat was recorded: silence now counts from it, not from the start
        r.wd.lock().unwrap().tick(1_000 + 179);
        assert!(!r.wd.lock().unwrap().is_open(crate::watchdog::Component::Pc));
        r.srv.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn a_wrong_or_missing_key_is_401_and_feeds_nothing() {
        let r = rig().await;
        for auth in [
            None,
            Some("Bearer nope"),
            Some(KEY),
            Some("Basic abc"),
            Some("Bearer "),
        ] {
            assert_eq!(post(&r, auth, DOWN).await, 401, "{auth:?}");
        }
        for _ in 0..3 {
            post(&r, Some("Bearer nope"), DOWN).await;
        }
        assert!(r.got.lock().unwrap().is_empty());
        assert!(
            !r.wd
                .lock()
                .unwrap()
                .is_open(crate::watchdog::Component::Synapse)
        );
        r.srv.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn anything_but_the_fixed_shape_is_400_and_feeds_nothing() {
        let r = rig().await;
        for body in [
            "",
            "{}",
            r#"{"synapse":"up","canary":"ok","bridge":"up","text":"hi"}"#,
            r#"{"synapse":"sideways","canary":"ok","bridge":"up"}"#,
        ] {
            assert_eq!(post(&r, Some(&bearer()), body).await, 400, "{body}");
        }
        let big = "x".repeat(MAX_HEARTBEAT_BODY_BYTES + 1);
        assert_eq!(post(&r, Some(&bearer()), &big).await, 413);
        r.srv.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn three_down_beats_in_a_row_hand_a_page_to_the_sink() {
        let r = rig().await;
        for i in 0..3 {
            r.now.store(1_000 + i * 60, Ordering::SeqCst);
            assert_eq!(post(&r, Some(&bearer()), DOWN).await, 200);
        }
        let got = r.got.lock().unwrap().clone();
        assert_eq!(got.len(), 1);
        assert!(
            matches!(&got[0], Action::Page { body, .. } if body.starts_with("Synapse is down since"))
        );
        r.srv.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn a_flood_is_429_and_the_extra_beats_are_not_counted() {
        let r = rig().await;
        for _ in 0..MAX_BEATS_PER_MINUTE {
            assert_eq!(post(&r, Some(&bearer()), DOWN).await, 200);
        }
        // ten bad beats in one second already paged once; the eleventh is refused
        assert_eq!(post(&r, Some(&bearer()), DOWN).await, 429);
        r.now.store(1_000 + 60, Ordering::SeqCst);
        assert_eq!(
            post(&r, Some(&bearer()), GOOD).await,
            200,
            "the window rolls"
        );
        r.srv.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn only_post_is_routed() {
        let r = rig().await;
        let s = Client::new()
            .get(format!("http://{}/heartbeat", r.srv.addr()))
            .header("authorization", bearer())
            .send()
            .await
            .unwrap()
            .status()
            .as_u16();
        assert!(matches!(s, 404 | 405), "{s}");
        r.srv.shutdown().await.unwrap();
    }
}
