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

use crate::Stage;
use crate::ack::{AckToken, AlertStore, bearer_token};
use crate::channel::{Dispatcher, PcAlarmBoard, PcAlarmConfig, drain_second, pc_alarm_route};
use crate::enroll::EnrollmentStore;
use crate::escalate::Escalator;
use crate::fcm::PushTransport;
use crate::raise::{DeviceRegistry, response};
use crate::ratelimit::AlertLimiter;
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
    /// The cap on `POST /alert` (ROADMAP A10).
    pub alert_limiter: Arc<AlertLimiter>,
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
            alert_limiter: Arc::new(AlertLimiter::default()),
            clock,
        }
    }
}

impl<T, D> App<T, D> {
    /// Replaces the caps on `POST /alert` (defaults: 6 an hour, 20 a day per key owner).
    pub fn with_alert_limits(self, limits: crate::ratelimit::RateLimits) -> Self {
        App {
            alert_limiter: Arc::new(AlertLimiter::new(limits)),
            ..self
        }
    }

    /// Replaces the in-memory escalator with one backed by `store`: open alerts resume (a timer that
    /// came due while down fires once on the next tick) and every alert already at the PC-alarm
    /// stage goes back on the board, so a restart does not silently end the chain. The second
    /// app's push is not repeated (a call already made is never made twice).
    pub fn with_escalations(
        self,
        minutes: u64,
        store: Box<dyn crate::escalate::EscalationStore>,
    ) -> Self {
        let escalator = Escalator::with_store(minutes, store);
        for id in escalator.at_stage(Stage::PcAlarm) {
            self.board.raise(&id);
        }
        App {
            escalator: Arc::new(Mutex::new(escalator)),
            ..self
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
            alerts: Arc::clone(&app.alerts),
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
    let (devices, transport, alerts, escalator, clock, alert_limiter) = (
        Arc::clone(&app.devices),
        Arc::clone(&app.transport),
        Arc::clone(&app.alerts),
        Arc::clone(&app.escalator),
        Arc::clone(&app.clock),
        Arc::clone(&app.alert_limiter),
    );
    server.post("/alert", move |req: Request| {
        let (api_key, devices, transport, alerts, escalator, clock, alert_limiter) = (
            Arc::clone(&api_key),
            Arc::clone(&devices),
            Arc::clone(&transport),
            Arc::clone(&alerts),
            Arc::clone(&escalator),
            Arc::clone(&clock),
            Arc::clone(&alert_limiter),
        );
        async move {
            let auth = authorization(&req).map(str::to_owned);
            // The rate limit comes AFTER authentication: a stranger's request is the usual 401 and spends
            // nobody's budget; only the key's owner is counted (ROADMAP A10). A refusal sends no push.
            let authenticated = auth
                .as_deref()
                .and_then(bearer_token)
                .is_some_and(|t| api_key.matches(t));
            if authenticated && !alert_limiter.allow(crate::ratelimit::DEFAULT_OWNER, clock()) {
                eprintln!("alarmant: alert refused (rate limit)");
                return json_response(
                    429,
                    &serde_json::json!({"error": "too many alerts; try again later"}),
                );
            }
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
    use crate::ratelimit::RateLimits;
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

    #[test]
    fn a_restart_puts_an_alert_already_at_the_pc_stage_back_on_the_board() {
        use crate::AlertState;
        use crate::ack::AlertRecord;
        use crate::escalate::{EscalationRecord, EscalationStore, MemoryEscalationStore};
        let rec = |id: &str, stage| EscalationRecord {
            alert_id: id.into(),
            state: AlertState::Escalated,
            stage,
            due: Some(10),
        };
        let store = Arc::new(MemoryEscalationStore::default());
        store.save(rec("at-pc", Stage::PcAlarm));
        store.save(rec("at-second", Stage::SecondApp));
        let app =
            app(Arc::new(AtomicU64::new(0))).with_escalations(1, Box::new(Arc::clone(&store)));
        let alerts = MemoryStore::default();
        for id in ["at-pc", "at-second"] {
            alerts.insert(AlertRecord {
                alert_id: id.into(),
                state: AlertState::Escalated,
                sent_at: 0,
                acked_at: None,
                ack_token: AckToken::new("ack1"),
                second_ack_token: None,
                text: None,
            });
        }
        assert_eq!(app.board.active(&alerts), vec!["at-pc".to_owned()]);
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

    /// ROADMAP A10: the N+1th alert inside the window is refused with a fixed message and sends no push;
    /// a wrong key neither raises nor spends the budget; the window moves with the clock.
    #[tokio::test]
    async fn the_alert_endpoint_is_rate_limited_per_key() {
        let now = Arc::new(AtomicU64::new(10_000));
        let app = app(Arc::clone(&now));
        let srv = public_server(&app).bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", srv.addr());
        let good = format!("Bearer {KEY}");
        let body = r#"{"title":"t","body":"b"}"#;
        // a stranger's calls are 401 and never spend the owner's budget, however many there are
        for _ in 0..30 {
            assert_eq!(
                call(&base, "POST", "/alert", "Bearer nope", body).await.0,
                401
            );
        }
        let per_hour = RateLimits::default().per_hour;
        for i in 0..per_hour {
            let (st, _) = call(&base, "POST", "/alert", &good, body).await;
            assert_eq!(st, 202, "alert {i} is inside the window");
        }
        let pushes = app.transport.0.lock().unwrap().len();
        let (st, v) = call(&base, "POST", "/alert", &good, body).await;
        assert_eq!(st, 429, "the N+1th alert in the hour is refused");
        assert_eq!(
            v["error"], "too many alerts; try again later",
            "fixed message"
        );
        assert_eq!(
            app.transport.0.lock().unwrap().len(),
            pushes,
            "a refused alert sends no push"
        );
        // an hour later the hourly window has moved on
        now.store(10_000 + 3_601, Ordering::SeqCst);
        assert_eq!(call(&base, "POST", "/alert", &good, body).await.0, 202);
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
        assert_eq!(app.alerts.get(&id).unwrap().state, crate::AlertState::Acked);
        assert!(app.escalator.lock().unwrap().state(&id).is_none());
    }
}
