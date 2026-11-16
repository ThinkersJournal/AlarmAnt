//! The running service: the public routes with the escalation timer wired in, and one timer step.
//!
//! [`public_server`] is `POST /alert` and `POST /ack/:id` through [`crate::wire`] (so a raised alert
//! starts its timer and an acknowledgement ends it), plus `POST /register`, `PUT /device/token` and
//! `GET /pc/alarm`. [`tick_once`] is one timer step: fire what is due, then send the queued second
//! contact pushes. The binary owns the clock and the loop; nothing here sleeps.

use std::sync::{Arc, Mutex};

use httpant_server::Server;
use httpant_server::core::http::StatusCode;
use httpant_server::core::{IntoResponse, PathParams, Request, Response};

use crate::ack::{AckToken, AlertStore, bearer_token};
use crate::channel::{Dispatcher, PcAlarmBoard, PcAlarmConfig, drain_second, pc_alarm_route};
use crate::enroll::EnrollmentStore;
use crate::escalate::Escalator;
use crate::fcm::PushTransport;
use crate::raise::{DeviceRegistry, response};
use crate::register::{DeviceStore, FailureLimiter};
use crate::routes::{
    Clock, MAX_RAISE_BODY_BYTES, RegisterConfig, authorization, device_token_route, json_response,
    register_route,
};
use crate::second::SecondError;
use crate::wire::{acknowledge_escalating, raise_escalating};

/// Everything the service shares between its routes and its timer.
pub struct App<T, D> {
    pub api_key: AckToken,
    pub pc_key: AckToken,
    pub devices: Arc<D>,
    pub alerts: Arc<dyn AlertStore>,
    pub enrollments: Arc<dyn EnrollmentStore>,
    pub transport: Arc<T>,
    pub escalator: Arc<Mutex<Escalator>>,
    pub dispatcher: Arc<Dispatcher>,
    pub board: Arc<PcAlarmBoard>,
    pub limiter: Arc<FailureLimiter>,
    pub clock: Clock,
}

impl<T, D> App<T, D> {
    /// A new app with an in-memory escalator of `minutes` per step.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        api_key: AckToken,
        pc_key: AckToken,
        devices: Arc<D>,
        alerts: Arc<dyn AlertStore>,
        enrollments: Arc<dyn EnrollmentStore>,
        transport: Arc<T>,
        minutes: u64,
        clock: Clock,
    ) -> Self {
        let board = Arc::new(PcAlarmBoard::default());
        App {
            api_key,
            pc_key,
            devices,
            alerts,
            enrollments,
            transport,
            escalator: Arc::new(Mutex::new(Escalator::new(minutes))),
            dispatcher: Arc::new(Dispatcher::new(Arc::clone(&board))),
            board,
            limiter: Arc::new(FailureLimiter::default()),
            clock,
        }
    }
}

/// Adds the public routes to a new server.
pub fn public_server<T, D>(app: &App<T, D>) -> Server
where
    T: PushTransport + Send + Sync + 'static,
    D: DeviceRegistry + DeviceStore + 'static,
{
    let server = alert_route(Server::new(), app);
    let server = ack_route(server, app);
    let devices: Arc<dyn DeviceStore> = app.devices.clone();
    let server = register_route(
        server,
        RegisterConfig {
            enrollments: Arc::clone(&app.enrollments),
            devices: Arc::clone(&devices),
            limiter: Arc::clone(&app.limiter),
            clock: Arc::clone(&app.clock),
        },
    );
    let server = device_token_route(server, devices);
    let server = pc_ack_route(server, app);
    pc_alarm_route(
        server,
        PcAlarmConfig {
            pc_key: app.pc_key.clone(),
            board: Arc::clone(&app.board),
            alerts: Arc::clone(&app.alerts),
        },
    )
}

fn alert_route<T, D>(server: Server, app: &App<T, D>) -> Server
where
    T: PushTransport + Send + Sync + 'static,
    D: DeviceRegistry + 'static,
{
    let api_key = Arc::new(app.api_key.clone());
    let (devices, transport, alerts, escalator, clock) = (
        Arc::clone(&app.devices),
        Arc::clone(&app.transport),
        Arc::clone(&app.alerts),
        Arc::clone(&app.escalator),
        Arc::clone(&app.clock),
    );
    server.post("/alert", move |req: Request| {
        let (api_key, devices, transport, alerts, escalator, clock) = (
            Arc::clone(&api_key),
            Arc::clone(&devices),
            Arc::clone(&transport),
            Arc::clone(&alerts),
            Arc::clone(&escalator),
            Arc::clone(&clock),
        );
        async move {
            let auth = authorization(&req).map(str::to_owned);
            let Ok(bytes) = req.into_body().bytes(MAX_RAISE_BODY_BYTES).await else {
                return json_response(413, &serde_json::json!({"error": "body too large"}));
            };
            let Ok(text) = std::str::from_utf8(&bytes) else {
                return json_response(400, &serde_json::json!({"error": "body is not UTF-8"}));
            };
            let result = raise_escalating(
                &escalator,
                &api_key,
                auth.as_deref(),
                text,
                &*devices,
                &*transport,
                &*alerts,
                clock(),
            )
            .await;
            let (status, body) = response(&result);
            json_response(status, &body)
        }
    })
}

fn ack_route<T, D>(server: Server, app: &App<T, D>) -> Server {
    let (alerts, escalator, dispatcher, clock) = (
        Arc::clone(&app.alerts),
        Arc::clone(&app.escalator),
        Arc::clone(&app.dispatcher),
        Arc::clone(&app.clock),
    );
    server.post("/ack/:alert_id", move |req: Request| {
        let (alerts, escalator, dispatcher, clock) = (
            Arc::clone(&alerts),
            Arc::clone(&escalator),
            Arc::clone(&dispatcher),
            Arc::clone(&clock),
        );
        async move {
            let id = req
                .extensions()
                .get::<PathParams>()
                .and_then(|p| p.get("alert_id"))
                .map(str::to_owned);
            let Some(id) = id else {
                return StatusCode::NOT_FOUND.into_response();
            };
            let (outcome, _ended) = acknowledge_escalating(
                &escalator,
                &*dispatcher,
                &*alerts,
                &id,
                authorization(&req),
                clock(),
            );
            let status = StatusCode::from_u16(outcome.status()).unwrap_or(StatusCode::NOT_FOUND);
            let res: Response = status.into_response();
            res
        }
    })
}

/// Adds `POST /pc/ack/:alert_id`: the PC program's Acknowledge (its one outgoing message,
/// constraint 18). `Authorization: Bearer <PC key>`; accepted only for an alert whose PC alarm is
/// sounding, so the PC key cannot touch any other alert. Ends the alert for everyone, like any
/// acknowledgement. 401 for a bad key, 404 for any alert that is not at the PC step.
fn pc_ack_route<T, D>(server: Server, app: &App<T, D>) -> Server {
    let (key, alerts, board, escalator, dispatcher, clock) = (
        Arc::new(app.pc_key.clone()),
        Arc::clone(&app.alerts),
        Arc::clone(&app.board),
        Arc::clone(&app.escalator),
        Arc::clone(&app.dispatcher),
        Arc::clone(&app.clock),
    );
    server.post("/pc/ack/:alert_id", move |req: Request| {
        let (key, alerts, board, escalator, dispatcher, clock) = (
            Arc::clone(&key),
            Arc::clone(&alerts),
            Arc::clone(&board),
            Arc::clone(&escalator),
            Arc::clone(&dispatcher),
            Arc::clone(&clock),
        );
        async move {
            let ok = authorization(&req)
                .and_then(bearer_token)
                .is_some_and(|k| key.matches(k));
            if !ok {
                return StatusCode::UNAUTHORIZED.into_response();
            }
            let id = req
                .extensions()
                .get::<PathParams>()
                .and_then(|p| p.get("alert_id"))
                .map(str::to_owned);
            let Some(id) = id.filter(|id| board.active(&*alerts).contains(id)) else {
                return StatusCode::NOT_FOUND.into_response();
            };
            alerts.mark_acked(&id, clock());
            escalator
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .ack(&id, &*dispatcher);
            let res: Response = StatusCode::OK.into_response();
            res
        }
    })
}

/// One timer step at `now`: fires every due timeout, then sends the queued second-contact pushes.
/// Returns how each second push went, for the audit trail.
pub async fn tick_once<T, D>(app: &App<T, D>, now: u64) -> Vec<(String, Result<(), SecondError>)>
where
    T: PushTransport + Send + Sync + 'static,
    D: DeviceRegistry + 'static,
{
    {
        let mut e = app.escalator.lock().unwrap_or_else(|p| p.into_inner());
        e.tick(now, &*app.dispatcher);
    }
    drain_second(
        &app.dispatcher,
        &*app.devices,
        &*app.transport,
        &*app.alerts,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ack::MemoryStore;
    use crate::enroll::MemoryEnrollments;
    use crate::fcm::{AlertMessage, DeviceToken, PushError, PushReceipt};
    use crate::raise::Device;
    use crate::register::MemoryDevices;
    use httpant_client::Client;
    use std::future::Future;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[derive(Default)]
    struct Sent(Mutex<Vec<String>>);
    #[allow(clippy::manual_async_fn)]
    impl PushTransport for Sent {
        fn send(
            &self,
            t: &DeviceToken,
            m: &AlertMessage,
        ) -> impl Future<Output = Result<PushReceipt, PushError>> + Send {
            self.0
                .lock()
                .unwrap()
                .push(format!("{}:{}", t.expose(), m.alert_id));
            async {
                Ok(PushReceipt {
                    message_name: "m".into(),
                })
            }
        }
    }

    const KEY: &str = "api-key-0123456789-0123456789-0123456789";

    fn app(now: Arc<AtomicU64>) -> App<Sent, MemoryDevices> {
        let devices = Arc::new(MemoryDevices::default());
        devices.set_for(
            crate::enroll::Contact::First,
            Device {
                push_token: DeviceToken::new("p1"),
                ack_token: AckToken::new("ack1"),
            },
        );
        devices.set_for(
            crate::enroll::Contact::Second,
            Device {
                push_token: DeviceToken::new("p2"),
                ack_token: AckToken::new("ack2"),
            },
        );
        App::new(
            AckToken::new(KEY),
            AckToken::new("pc-key-0123456789"),
            devices,
            Arc::new(MemoryStore::default()),
            Arc::new(MemoryEnrollments::default()),
            Arc::new(Sent::default()),
            1,
            Arc::new(move || now.load(Ordering::SeqCst)),
        )
    }

    async fn call(
        base: &str,
        method: &str,
        path: &str,
        auth: &str,
        body: &str,
    ) -> (u16, serde_json::Value) {
        let c = Client::new();
        let url = format!("{base}{path}");
        let r = if method == "GET" {
            c.get(url)
        } else {
            c.post(url).body(body.to_owned())
        };
        let res = r.header("authorization", auth).send().await.unwrap();
        let st = res.status().as_u16();
        let t = res.text().await.unwrap();
        (
            st,
            serde_json::from_str(&t).unwrap_or(serde_json::Value::Null),
        )
    }

    #[tokio::test]
    async fn timer_escalates_to_second_app_then_pc_and_an_ack_ends_it() {
        let now = Arc::new(AtomicU64::new(1000));
        let app = app(Arc::clone(&now));
        let srv = public_server(&app).bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", srv.addr());
        let bearer = format!("Bearer {KEY}");
        let pc = "Bearer pc-key-0123456789";

        let (st, v) = call(
            &base,
            "POST",
            "/alert",
            &bearer,
            r#"{"title":"t","body":"b"}"#,
        )
        .await;
        assert_eq!(st, 202);
        let id = v["alert_id"].as_str().unwrap().to_owned();
        assert_eq!(app.transport.0.lock().unwrap().len(), 1, "first push");

        // Not yet due.
        now.store(1030, Ordering::SeqCst);
        assert!(tick_once(&app, 1030).await.is_empty());
        // First timeout: second contact's app.
        now.store(1060, Ordering::SeqCst);
        let r = tick_once(&app, 1060).await;
        assert_eq!(r, vec![(id.clone(), Ok(()))]);
        assert_eq!(app.transport.0.lock().unwrap()[1], format!("p2:{id}"));
        let (_, v) = call(&base, "GET", "/pc/alarm", pc, "").await;
        assert_eq!(v["alert_ids"].as_array().unwrap().len(), 0);
        // Second timeout: the PC alarm.
        now.store(1120, Ordering::SeqCst);
        tick_once(&app, 1120).await;
        let (_, v) = call(&base, "GET", "/pc/alarm", pc, "").await;
        assert_eq!(v["alert_ids"][0], id.as_str());
        // Either contact's ack ends it.
        let (st, _) = call(&base, "POST", &format!("/ack/{id}"), "Bearer ack2", "").await;
        assert_eq!(st, 200);
        let (_, v) = call(&base, "GET", "/pc/alarm", pc, "").await;
        assert!(v["alert_ids"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn an_acked_alert_never_reaches_the_second_contact() {
        let now = Arc::new(AtomicU64::new(1000));
        let app = app(Arc::clone(&now));
        let srv = public_server(&app).bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", srv.addr());
        let (_, v) = call(
            &base,
            "POST",
            "/alert",
            &format!("Bearer {KEY}"),
            r#"{"title":"t","body":"b"}"#,
        )
        .await;
        let id = v["alert_id"].as_str().unwrap().to_owned();
        let (st, _) = call(&base, "POST", &format!("/ack/{id}"), "Bearer ack1", "").await;
        assert_eq!(st, 200);
        now.store(2000, Ordering::SeqCst);
        assert!(tick_once(&app, 2000).await.is_empty());
        assert_eq!(app.transport.0.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn the_pc_key_acks_only_an_alert_at_the_pc_step() {
        let now = Arc::new(AtomicU64::new(1000));
        let app = app(Arc::clone(&now));
        let srv = public_server(&app).bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", srv.addr());
        let (_, v) = call(
            &base,
            "POST",
            "/alert",
            &format!("Bearer {KEY}"),
            r#"{"title":"t","body":"b"}"#,
        )
        .await;
        let id = v["alert_id"].as_str().unwrap().to_owned();
        let path = format!("/pc/ack/{id}");
        let pc = "Bearer pc-key-0123456789";
        // Not at the PC step yet: refused, and the alert stays open.
        assert_eq!(call(&base, "POST", &path, pc, "").await.0, 404);
        for t in [1060, 1120] {
            now.store(t, Ordering::SeqCst);
            tick_once(&app, t).await;
        }
        assert_eq!(call(&base, "POST", &path, "Bearer wrong", "").await.0, 401);
        assert_eq!(call(&base, "POST", &path, "", "").await.0, 401);
        let (_, v) = call(&base, "GET", "/pc/alarm", pc, "").await;
        assert_eq!(v["alert_ids"][0], id.as_str(), "bad keys changed nothing");
        assert_eq!(call(&base, "POST", &path, pc, "").await.0, 200);
        let (_, v) = call(&base, "GET", "/pc/alarm", pc, "").await;
        assert!(v["alert_ids"].as_array().unwrap().is_empty());
        assert_eq!(
            app.alerts.get(&id).unwrap().state,
            crate::AlertState::Acked
        );
        assert!(app.escalator.lock().unwrap().state(&id).is_none());
    }
}
