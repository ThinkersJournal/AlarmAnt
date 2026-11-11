//! The HTTP routes, on HTTPAnt's server half. Each route is a small function that adds itself to a
//! [`Server`]; the handlers hold no logic of their own, they pass the header, the path and the clock
//! to the framework-free handlers ([`crate::ack`], later [`crate::raise`]).

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use httpant_server::Server;
use httpant_server::core::http::StatusCode;
use httpant_server::core::http::header::AUTHORIZATION;
use httpant_server::core::{IntoResponse, PathParams, Request, Response};

use crate::ack::{AlertStore, acknowledge};

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
}
