//! The watchdog's running parts (docs/SYNAPSE-ADAPTER.md 5): the shared state, the page step that
//! raises a watchdog page through the NORMAL alert path (first app, second app, PC alarm), the
//! acknowledgement read-back that starts the 30-minute quiet, and the operator's maintenance pause
//! (`POST /admin/watchdog/pause` on the loopback admin server, `alarmant watchdog pause`).
//!
//! A watchdog page is an ordinary alert with fixed text. Nothing here gives the escalator a new
//! input: the page is a raise, the human's Acknowledge is read from the alert store, and the timer
//! is the escalator's own (PATENT-SWEEP 13-22).

use std::sync::{Arc, Mutex};

use httpant_client::Client;
use httpant_server::Server;
use httpant_server::core::{Request, Response};

use crate::AlertState;
use crate::ack::{AckToken, AlertStore, bearer_token};
use crate::adminapi::{MAX_ADMIN_BODY_BYTES, is_loopback_base};
use crate::escalate::Escalator;
use crate::fcm::PushTransport;
use crate::heartbeat::{ActionSink, HeartbeatConfig};
use crate::raise::{DeviceRegistry, raise_authorised};
use crate::routes::{Clock, authorization, json_response};
use crate::watchdog::{Action, Component, PauseError, Watchdog};

/// The watchdog and what it has asked for but the step has not yet done.
pub struct WatchState {
    /// The heartbeat key (its own; never the API key).
    pub key: AckToken,
    pub wd: Arc<Mutex<Watchdog>>,
    queue: Arc<Mutex<Vec<Action>>>,
    /// The open watchdog page of each component, to read its Acknowledge back.
    open: Mutex<Vec<(Component, String)>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl WatchState {
    /// A watchdog started at `now` (the restart grace runs from here).
    pub fn new(key: AckToken, now: u64) -> Self {
        WatchState {
            key,
            wd: Arc::new(Mutex::new(Watchdog::new(now))),
            queue: Arc::default(),
            open: Mutex::default(),
        }
    }

    /// Where the heartbeat route hands the machine's actions: the step's queue.
    pub fn sink(&self) -> ActionSink {
        let q = Arc::clone(&self.queue);
        Arc::new(move |a| lock(&q).extend(a))
    }

    /// The configuration for [`crate::heartbeat::heartbeat_route`].
    pub fn heartbeat_config(&self, clock: Clock) -> HeartbeatConfig {
        HeartbeatConfig {
            key: self.key.clone(),
            watchdog: Arc::clone(&self.wd),
            sink: self.sink(),
            clock,
        }
    }

    /// Starts a maintenance pause (audited on stderr; expires by itself).
    pub fn pause(&self, minutes: u64, now: u64) -> Result<u64, PauseError> {
        let end = lock(&self.wd).pause(minutes, now)?;
        eprintln!("alarmant: watchdog paused for {minutes} min");
        Ok(end)
    }
}

/// One watchdog step at `now`: read acknowledgements back, run the timer, and raise whatever page is
/// due through the normal chain. Call once per service tick.
pub async fn watchdog_step<T>(
    ws: &WatchState,
    devices: &dyn DeviceRegistry,
    transport: &T,
    alerts: &dyn AlertStore,
    escalator: &Mutex<Escalator>,
    now: u64,
) where
    T: PushTransport + Send + Sync,
{
    // A human acknowledged a watchdog page: start that component's quiet.
    let acked: Vec<Component> = {
        let mut open = lock(&ws.open);
        let mut done = Vec::new();
        open.retain(|(c, id)| {
            let acked = alerts.get(id).is_some_and(|r| r.state == AlertState::Acked);
            if acked {
                done.push(*c);
            }
            !acked
        });
        done
    };
    {
        let mut wd = lock(&ws.wd);
        for c in acked {
            wd.acknowledged(c, now);
        }
    }
    let mut actions = std::mem::take(&mut *lock(&ws.queue));
    actions.extend(lock(&ws.wd).tick(now));
    for action in actions {
        match action {
            Action::Suppressed { component, why } => {
                eprintln!("alarmant: watchdog page held back component={component:?} why={why:?}");
            }
            Action::Page {
                component,
                title,
                body,
            } => {
                let text = serde_json::json!({ "title": title, "body": body }).to_string();
                match raise_authorised(&text, devices, transport, alerts, now).await {
                    Ok(id) => {
                        let mut e = lock(escalator);
                        if !alerts
                            .get(&id)
                            .is_some_and(|r| r.state == AlertState::Acked)
                        {
                            e.register(&id, now);
                        }
                        lock(&ws.open).retain(|(c, _)| *c != component);
                        lock(&ws.open).push((component, id));
                    }
                    Err(e) => {
                        eprintln!("alarmant: watchdog page could not be raised: {e:?}")
                    }
                }
            }
        }
    }
}

fn minutes_of(text: &str) -> Option<u64> {
    serde_json::from_str::<serde_json::Value>(text)
        .ok()?
        .get("minutes")?
        .as_u64()
}

/// Adds `POST /admin/watchdog/pause` `{"minutes":N}` (1 to 60) to the loopback admin server.
/// 401 for a bad key (checked first), 400 for anything else wrong; 200 `{"text"}` when it took.
pub fn watchdog_admin_routes(
    server: Server,
    admin_key: AckToken,
    ws: Arc<WatchState>,
    clock: Clock,
) -> Server {
    let admin_key = Arc::new(admin_key);
    server.post("/admin/watchdog/pause", move |req: Request| {
        let (key, ws, clock) = (
            Arc::clone(&admin_key),
            Arc::clone(&ws),
            Arc::clone(&clock),
        );
        async move {
            let ok = authorization(&req)
                .and_then(bearer_token)
                .is_some_and(|k| key.matches(k));
            if !ok {
                return json_response(401, &serde_json::json!({"error": "unauthorized"}));
            }
            let Ok(bytes) = req.into_body().bytes(MAX_ADMIN_BODY_BYTES).await else {
                return json_response(413, &serde_json::json!({"error": "body too large"}));
            };
            let minutes = std::str::from_utf8(&bytes).ok().and_then(minutes_of);
            let Some(minutes) = minutes else {
                return json_response(400, &serde_json::json!({"error": "minutes required"}));
            };
            let res: Response = match ws.pause(minutes, clock()) {
                Ok(_) => json_response(
                    200,
                    &serde_json::json!({"text": format!("Watchdog paused for {minutes} minutes.\n")}),
                ),
                Err(PauseError::OutOfRange) => {
                    json_response(400, &serde_json::json!({"error": "minutes must be 1 to 60"}))
                }
            };
            res
        }
    })
}

/// The client side of the pause: `alarmant watchdog pause --minutes N`. Loopback only.
pub async fn send_pause(base: &str, key: &str, minutes: u64) -> Result<String, &'static str> {
    if !is_loopback_base(base) {
        return Err("the admin address must be http on a loopback host");
    }
    let res = Client::new()
        .post(format!(
            "{}/admin/watchdog/pause",
            base.trim_end_matches('/')
        ))
        .header("authorization", format!("Bearer {key}"))
        .body(format!(r#"{{"minutes":{minutes}}}"#))
        .send()
        .await
        .map_err(|_| "the service could not be reached")?;
    match res.status().as_u16() {
        200 => Ok(format!("Watchdog paused for {minutes} minutes.\n")),
        401 => Err("the admin key was refused"),
        400 => Err("the pause was refused (minutes must be 1 to 60)"),
        _ => Err("the service refused the command"),
    }
}

/// Parses `watchdog pause --minutes N`.
pub fn parse_pause(args: &[String]) -> Option<u64> {
    match args {
        [w, p, f, n] if w == "watchdog" && p == "pause" && f == "--minutes" => n.parse().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ack::MemoryStore;
    use crate::enroll::Contact;
    use crate::fcm::{AlertMessage, DeviceToken, PushError, PushReceipt};
    use crate::raise::Device;
    use crate::register::{DeviceStore, MemoryDevices};
    use crate::watchdog::{Heartbeat, QUIET_AFTER_ACK};
    use std::future::Future;

    const T0: u64 = 1_000_000;
    const DOWN: Heartbeat = Heartbeat {
        synapse_up: false,
        canary_ok: true,
    };

    #[derive(Default)]
    struct Sent(Mutex<Vec<AlertMessage>>);
    #[allow(clippy::manual_async_fn)]
    impl PushTransport for Sent {
        fn send(
            &self,
            _: &DeviceToken,
            m: &AlertMessage,
        ) -> impl Future<Output = Result<PushReceipt, PushError>> + Send {
            lock(&self.0).push(m.clone());
            async {
                Ok(PushReceipt {
                    message_name: "m".into(),
                })
            }
        }
    }

    struct Rig {
        ws: WatchState,
        devices: MemoryDevices,
        transport: Sent,
        alerts: MemoryStore,
        esc: Mutex<Escalator>,
    }

    fn rig() -> Rig {
        let devices = MemoryDevices::default();
        devices.set_for(
            Contact::First,
            Device {
                push_token: DeviceToken::new("p1"),
                ack_token: AckToken::new("ack1"),
            },
        );
        Rig {
            ws: WatchState::new(AckToken::new("hb-key-0123456789-0123456789-0123"), T0),
            devices,
            transport: Sent::default(),
            alerts: MemoryStore::default(),
            esc: Mutex::new(Escalator::new(5)),
        }
    }

    impl Rig {
        fn beat(&self, now: u64, hb: Heartbeat) {
            let a = lock(&self.ws.wd).beat(now, hb);
            (self.ws.sink())(a);
        }
        async fn step(&self, now: u64) {
            watchdog_step(
                &self.ws,
                &self.devices,
                &self.transport,
                &self.alerts,
                &self.esc,
                now,
            )
            .await;
        }
        fn sent(&self) -> Vec<AlertMessage> {
            lock(&self.transport.0).clone()
        }
    }

    #[tokio::test]
    async fn a_page_is_a_normal_alert_with_fixed_text_and_a_running_chain() {
        let r = rig();
        for i in 0..3 {
            r.beat(T0 + i * 60, DOWN);
        }
        r.step(T0 + 120).await;
        let sent = r.sent();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].title, "AlarmAnt watchdog");
        assert!(
            sent[0].body.starts_with("Synapse is down since"),
            "{}",
            sent[0].body
        );
        assert!(
            lock(&r.esc).state(&sent[0].alert_id).is_some(),
            "the normal escalation timer runs for it"
        );
        r.step(T0 + 180).await;
        assert_eq!(r.sent().len(), 1, "one open alert: no repeat");
    }

    #[tokio::test]
    async fn silence_pages_through_the_timer_alone() {
        let r = rig();
        r.step(T0 + 179).await;
        assert!(r.sent().is_empty());
        r.step(T0 + 180).await;
        let sent = r.sent();
        assert_eq!(sent.len(), 1);
        assert!(sent[0].body.starts_with("The PC stopped reporting at"));
    }

    #[tokio::test]
    async fn an_acknowledgement_starts_the_quiet_then_a_lasting_fault_pages_again() {
        let r = rig();
        for i in 0..3 {
            r.beat(T0 + i * 60, DOWN);
        }
        r.step(T0 + 120).await;
        let id = r.sent()[0].alert_id.clone();
        r.alerts.mark_acked(&id, T0 + 130);
        r.step(T0 + 130).await;
        let quiet_ends = T0 + 130 + QUIET_AFTER_ACK;
        for t in (T0 + 180..quiet_ends).step_by(60) {
            r.beat(t, DOWN);
            r.step(t).await;
        }
        assert_eq!(r.sent().len(), 1, "quiet for 30 minutes");
        r.beat(quiet_ends, DOWN);
        r.step(quiet_ends).await;
        assert_eq!(r.sent().len(), 2, "the fault lasted: paged again");
    }

    #[tokio::test]
    async fn no_device_means_no_page_and_no_crash() {
        let mut r = rig();
        r.devices = MemoryDevices::default();
        r.step(T0 + 180).await;
        assert!(r.sent().is_empty());
    }

    #[test]
    fn pause_arguments_parse_strictly() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            parse_pause(&a(&["watchdog", "pause", "--minutes", "30"])),
            Some(30)
        );
        for bad in [
            &["watchdog", "pause"][..],
            &["watchdog", "pause", "--minutes", "x"],
            &["watchdog", "resume", "--minutes", "5"],
            &["watchdog", "pause", "--minutes", "-5"],
        ] {
            assert_eq!(parse_pause(&a(bad)), None, "{bad:?}");
        }
    }

    #[tokio::test]
    async fn the_pause_route_checks_the_key_then_the_range_and_pauses() {
        let ws = Arc::new(WatchState::new(
            AckToken::new("hb-key-0123456789-0123456789-0123"),
            T0,
        ));
        let srv = watchdog_admin_routes(
            Server::new(),
            AckToken::new("admin-key-0123456789-0123456789-01"),
            Arc::clone(&ws),
            Arc::new(|| T0 + 5),
        )
        .bind("127.0.0.1:0")
        .await
        .unwrap();
        let base = format!("http://{}", srv.addr());
        let post = |auth: &'static str, body: &'static str| {
            let url = format!("{base}/admin/watchdog/pause");
            async move {
                Client::new()
                    .post(url)
                    .header("authorization", auth)
                    .body(body)
                    .send()
                    .await
                    .unwrap()
                    .status()
                    .as_u16()
            }
        };
        let good = "Bearer admin-key-0123456789-0123456789-01";
        assert_eq!(post("Bearer nope", r#"{"minutes":5}"#).await, 401);
        assert_eq!(
            lock(&ws.wd).paused_until(),
            None,
            "a wrong key changes nothing"
        );
        for body in [
            r#"{"minutes":0}"#,
            r#"{"minutes":61}"#,
            r#"{"minutes":"5"}"#,
            "{}",
        ] {
            assert_eq!(post(good, body).await, 400, "{body}");
        }
        assert_eq!(lock(&ws.wd).paused_until(), None);
        assert_eq!(post(good, r#"{"minutes":5}"#).await, 200);
        assert_eq!(lock(&ws.wd).paused_until(), Some(T0 + 5 + 300));
        let base2 = base.clone();
        assert_eq!(
            send_pause(&base2, "admin-key-0123456789-0123456789-01", 7).await,
            Ok("Watchdog paused for 7 minutes.\n".to_owned())
        );
        assert!(send_pause("http://example.com:9100", "k", 5).await.is_err());
        srv.shutdown().await.unwrap();
    }
}
