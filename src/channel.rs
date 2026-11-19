//! The server's [`AlertChannel`] (ruling 389, tiers 2 and 3) and the PC-alarm board.
//!
//! [`AlertChannel`] is synchronous and runs under the escalator's lock, so [`Dispatcher`] only
//! records what is due: the second app's push goes on a queue that [`drain_second`] sends from the
//! async side, and the PC alarm is a flag on the [`PcAlarmBoard`] that the PC program polls
//! (`GET /pc/alarm`). Nothing here waits, retries or reads a reply; the only input is still an
//! acknowledgement, which ends both (the board is read against the alert store, so an acked alert
//! drops off it). Nothing here logs or carries a token.
//!
//! After a restart the served app rebuilds the board from the persisted escalation records
//! (`App::with_escalations`). The second-push queue is still in memory: it is drained right after
//! each tick, and a push already made is never repeated.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use crate::ack::{AckToken, AlertStore, bearer_token};
use crate::fcm::PushTransport;
use crate::raise::DeviceRegistry;
use crate::routes::{authorization, json_response};
use crate::second::{SecondError, push_second};
use crate::{AlertChannel, AlertState, ChannelError};
use httpant_server::Server;
use httpant_server::core::Request;
use httpant_server::core::http::header::{CACHE_CONTROL, HeaderValue};

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Alerts whose PC alarm has been raised.
#[derive(Default)]
pub struct PcAlarmBoard {
    raised: Mutex<BTreeSet<String>>,
}

impl PcAlarmBoard {
    pub fn raise(&self, alert_id: &str) {
        lock(&self.raised).insert(alert_id.to_owned());
    }

    /// Alert ids that should be sounding now: raised, and still unacknowledged in `store`. Acked
    /// and unknown ones are dropped from the board.
    pub fn active(&self, store: &dyn AlertStore) -> Vec<String> {
        let mut raised = lock(&self.raised);
        raised.retain(|id| store.get(id).is_some_and(|r| r.state != AlertState::Acked));
        raised.iter().cloned().collect()
    }
}

/// The [`AlertChannel`] the server runs: queue the second push, raise the PC alarm.
pub struct Dispatcher {
    queue: Mutex<Vec<String>>,
    board: Arc<PcAlarmBoard>,
}

impl Dispatcher {
    pub fn new(board: Arc<PcAlarmBoard>) -> Self {
        Dispatcher {
            queue: Mutex::new(Vec::new()),
            board,
        }
    }

    fn take_pending(&self) -> Vec<String> {
        std::mem::take(&mut *lock(&self.queue))
    }
}

impl AlertChannel for Dispatcher {
    fn second_app(&self, alert_id: &str) -> Result<(), ChannelError> {
        lock(&self.queue).push(alert_id.to_owned());
        Ok(())
    }

    fn pc_alarm(&self, alert_id: &str) -> Result<(), ChannelError> {
        self.board.raise(alert_id);
        Ok(())
    }
}

/// Sends every queued second-contact push; returns each alert id with how it went (for the audit
/// trail). Call it after each escalator tick.
pub async fn drain_second(
    dispatcher: &Dispatcher,
    devices: &dyn DeviceRegistry,
    transport: &impl PushTransport,
    store: &dyn AlertStore,
) -> Vec<(String, Result<(), SecondError>)> {
    let mut done = Vec::new();
    for id in dispatcher.take_pending() {
        let r = push_second(devices, transport, store, &id).await;
        done.push((id, r));
    }
    done
}

/// What `GET /pc/alarm` needs.
pub struct PcAlarmConfig {
    pub pc_key: AckToken,
    pub board: Arc<PcAlarmBoard>,
    pub alerts: Arc<dyn AlertStore>,
}

/// Adds `GET /pc/alarm`: `Authorization: Bearer <PC key>`; answers 200 `{"alert_ids":[...]}` (empty
/// when nothing should sound) or 401 with fixed text. The key is the PC program's own, separate
/// from the sender's API key and the admin key. Never cached.
pub fn pc_alarm_route(server: Server, cfg: PcAlarmConfig) -> Server {
    let cfg = Arc::new(cfg);
    server.get("/pc/alarm", move |req: Request| {
        let cfg = Arc::clone(&cfg);
        async move {
            let ok = authorization(&req)
                .and_then(bearer_token)
                .is_some_and(|k| cfg.pc_key.matches(k));
            let mut res = if ok {
                json_response(
                    200,
                    &serde_json::json!({"alert_ids": cfg.board.active(&*cfg.alerts)}),
                )
            } else {
                json_response(401, &serde_json::json!({"error": "unauthorized"}))
            };
            res.headers_mut()
                .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
            res
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ack::{AlertRecord, MemoryStore, acknowledge};
    use crate::fcm::{AlertMessage, DeviceToken, PushError, PushReceipt};
    use crate::raise::Device;
    use httpant_client::Client;
    use std::future::Future;

    fn store_with(id: &str) -> Arc<MemoryStore> {
        let s = Arc::new(MemoryStore::default());
        s.insert(AlertRecord::sent(id, 100, AckToken::new("tok1")));
        s
    }

    #[test]
    fn board_lists_raised_unacked_alerts_only() {
        let s = store_with("a");
        let b = PcAlarmBoard::default();
        assert!(b.active(&*s).is_empty());
        b.raise("a");
        b.raise("ghost");
        assert_eq!(b.active(&*s), vec!["a".to_owned()]);
        let _ = acknowledge(&*s, "a", Some("Bearer tok1"), 200);
        assert!(b.active(&*s).is_empty(), "an acked alert stops sounding");
    }

    #[test]
    fn pc_alarm_raises_the_board_and_second_app_queues() {
        let board = Arc::new(PcAlarmBoard::default());
        let d = Dispatcher::new(Arc::clone(&board));
        let s = store_with("a");
        d.pc_alarm("a").unwrap();
        assert_eq!(board.active(&*s), vec!["a".to_owned()]);
        d.second_app("a").unwrap();
        d.second_app("b").unwrap();
        assert_eq!(d.take_pending(), vec!["a".to_owned(), "b".to_owned()]);
        assert!(d.take_pending().is_empty(), "taken once");
    }

    struct Devs;
    impl DeviceRegistry for Devs {
        fn device(&self) -> Option<Device> {
            None
        }
        fn second_device(&self) -> Option<Device> {
            Some(Device {
                push_token: DeviceToken::new("p2"),
                ack_token: AckToken::new("tok2"),
            })
        }
    }

    struct Sent(Mutex<Vec<String>>);
    #[allow(clippy::manual_async_fn)]
    impl PushTransport for Sent {
        fn send(
            &self,
            t: &DeviceToken,
            m: &AlertMessage,
        ) -> impl Future<Output = Result<PushReceipt, PushError>> + Send {
            lock(&self.0).push(format!("{}:{}", t.expose(), m.alert_id));
            async {
                Ok(PushReceipt {
                    message_name: "m".into(),
                })
            }
        }
    }

    #[tokio::test]
    async fn drain_pushes_the_second_contact_once() {
        let s = Arc::new(MemoryStore::default());
        let mut rec = AlertRecord::sent("a", 100, AckToken::new("tok1"));
        rec.text = Some(crate::ack::AlertText {
            title: "t".into(),
            body: "b".into(),
        });
        s.insert(rec);
        let d = Dispatcher::new(Arc::default());
        d.second_app("a").unwrap();
        let tx = Sent(Mutex::new(Vec::new()));
        let r = drain_second(&d, &Devs, &tx, &*s).await;
        assert_eq!(r, vec![("a".to_owned(), Ok(()))]);
        assert_eq!(*lock(&tx.0), vec!["p2:a".to_owned()]);
        assert!(drain_second(&d, &Devs, &tx, &*s).await.is_empty());
    }

    async fn get(srv: &httpant_server::RunningServer, auth: Option<&str>) -> (u16, String) {
        let mut r = Client::new().get(format!("http://{}/pc/alarm", srv.addr()));
        if let Some(a) = auth {
            r = r.header("authorization", a);
        }
        let res = r.send().await.unwrap();
        (res.status().as_u16(), res.text().await.unwrap())
    }

    #[tokio::test]
    async fn pc_route_needs_the_pc_key() {
        let s = store_with("a");
        let board = Arc::new(PcAlarmBoard::default());
        board.raise("a");
        let srv = pc_alarm_route(
            Server::new(),
            PcAlarmConfig {
                pc_key: AckToken::new("pc-key-0123456789"),
                board,
                alerts: s,
            },
        )
        .bind("127.0.0.1:0")
        .await
        .unwrap();
        let (st, body) = get(&srv, None).await;
        assert_eq!((st, body.contains("\"a\"")), (401, false));
        let (st, body) = get(&srv, Some("Bearer wrong")).await;
        assert_eq!((st, body.contains("\"a\"")), (401, false));
        let (st, body) = get(&srv, Some("Bearer pc-key-0123456789")).await;
        assert_eq!((st, body.contains("\"a\"")), (200, true));
    }
}
