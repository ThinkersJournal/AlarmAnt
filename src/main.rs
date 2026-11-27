// SPDX-License-Identifier: MIT OR Apache-2.0
//! `alarmant serve` runs the service; `alarmant enroll|revoke --contact first|second` talks to a
//! running one over its loopback admin endpoint. Secrets come from the environment or a file,
//! never the command line, and no message here repeats one.
//!
//! Environment for `serve`: `ALARMANT_BIND` (public address, required), `ALARMANT_DATA_DIR`,
//! `ALARMANT_SERVICE_ACCOUNT_FILE`, `ALARMANT_API_KEY`, `ALARMANT_PC_KEY`, `ALARMANT_ADMIN_KEY` or
//! `ALARMANT_ADMIN_KEY_FILE`, optional `ALARMANT_ADMIN_ADDR` (loopback, default 127.0.0.1:9100) and
//! `ALARMANT_MINUTES` (default 5), `ALARMANT_HEARTBEAT_KEY` (turns the watchdog on: `POST /heartbeat`; must differ from every other key), `ALARMANT_RATE_PER_HOUR` and `ALARMANT_RATE_PER_DAY` (alerts a key owner may raise; default 6 and 20), and `ALARMANT_TLS_CERT_FILE` + `ALARMANT_TLS_KEY_FILE` (PEM; both or neither) to serve HTTPS itself.
//! `ALARMANT_TLS_SELF_SIGNED_NAMES` (IPs and host names, comma-separated) instead makes a self-signed pair in the data directory. For `enroll`, `ALARMANT_PUBLIC_URL`
//! (the `https://` address the phone uses) makes the client print the full enrol link, with the pin when a self-signed pair exists in `ALARMANT_DATA_DIR`. The client commands read the admin key and address the same way.

use std::net::SocketAddr;
use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use alarmant::ack::AckToken;
use alarmant::adminapi::{AdminConfig, admin_key_from_file, admin_routes, bind_loopback, send};
use alarmant::auth::ServiceAccount;
use alarmant::persist::{
    Disk, PersistedAlerts, PersistedDevices, PersistedEnrollments, PersistedEscalations,
};
use alarmant::push::HttpPushTransport;
use alarmant::routes::{api_key_from_value, system_clock};
use alarmant::serve::{App, public_server, tick_once};
use alarmant::wire::EscalationConfig;
use httpant_server::Server;

const DEFAULT_ADMIN_ADDR: &str = "127.0.0.1:9100";
const TICK: Duration = Duration::from_secs(5);

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

fn key(name: &str) -> Result<AckToken, String> {
    api_key_from_value(env(name).as_deref()).map_err(|_| format!("{name} is missing or too short"))
}

fn admin_key() -> Result<AckToken, String> {
    match env("ALARMANT_ADMIN_KEY_FILE") {
        Some(f) => admin_key_from_file(Path::new(&f))
            .map_err(|_| "the admin key file is missing or too short".to_owned()),
        None => key("ALARMANT_ADMIN_KEY"),
    }
}

/// The admin key as text, for the client commands.
fn admin_key_text() -> Result<String, String> {
    let text = match env("ALARMANT_ADMIN_KEY_FILE") {
        Some(f) => std::fs::read_to_string(f)
            .ok()
            .and_then(|t| t.lines().next().map(|l| l.trim().to_owned())),
        None => env("ALARMANT_ADMIN_KEY").map(|k| k.trim().to_owned()),
    };
    text.filter(|k| !k.is_empty())
        .ok_or_else(|| "set ALARMANT_ADMIN_KEY or ALARMANT_ADMIN_KEY_FILE".to_owned())
}

fn admin_addr() -> Result<SocketAddr, String> {
    env("ALARMANT_ADMIN_ADDR")
        .unwrap_or_else(|| DEFAULT_ADMIN_ADDR.to_owned())
        .parse()
        .map_err(|_| "ALARMANT_ADMIN_ADDR is not an address".to_owned())
}

async fn client(args: &[String]) -> Result<(), String> {
    let command = alarmant::admin::parse(args).map_err(|e| e.message().to_owned())?;
    let base = format!("http://{}", admin_addr()?);
    let text = send(&base, &admin_key_text()?, command)
        .await
        .map_err(|e| e.message().to_owned())?;
    print!("{text}");
    if let (true, Some(base), Some(code)) = (
        matches!(command, alarmant::admin::Command::Enroll(_)),
        env("ALARMANT_PUBLIC_URL"),
        alarmant::tls::code_in_reply(&text),
    ) {
        // The pin comes from the self-signed pair only when the server is configured to make one
        // (the same variable the server reads); a real certificate is never pinned.
        let self_signed_dir = match env("ALARMANT_TLS_SELF_SIGNED_NAMES") {
            None => None,
            Some(_) => Some(env("ALARMANT_DATA_DIR").ok_or(
                "ALARMANT_TLS_SELF_SIGNED_NAMES is set but ALARMANT_DATA_DIR is not: the link would carry no pin",
            )?),
        };
        let link = alarmant::tls::link_for(&base, code, self_signed_dir.as_deref().map(Path::new))?;
        println!(
            "Link for the phone:
{link}"
        );
    }
    Ok(())
}

/// `alarmant watchdog pause --minutes N`: a planned stop is not an incident (1 to 60 minutes, audited by the service).
async fn pause(args: &[String]) -> Result<(), String> {
    let minutes = alarmant::watchstate::parse_pause(args)
        .ok_or("usage: alarmant watchdog pause --minutes N (1 to 60)")?;
    let base = format!("http://{}", admin_addr()?);
    let text = alarmant::watchstate::send_pause(&base, &admin_key_text()?, minutes)
        .await
        .map_err(str::to_owned)?;
    print!("{text}");
    Ok(())
}

/// The caps on `POST /alert`: `ALARMANT_RATE_PER_HOUR` and `ALARMANT_RATE_PER_DAY`, default 6 and 20 (ROADMAP A10).
fn alert_limits() -> Result<alarmant::ratelimit::RateLimits, String> {
    let mut l = alarmant::ratelimit::RateLimits::default();
    for (name, slot) in [
        ("ALARMANT_RATE_PER_HOUR", &mut l.per_hour),
        ("ALARMANT_RATE_PER_DAY", &mut l.per_day),
    ] {
        if let Some(v) = env(name) {
            *slot = v
                .parse::<u32>()
                .ok()
                .filter(|n| *n > 0)
                .ok_or_else(|| format!("{name} must be a whole number above 0"))?;
        }
    }
    Ok(l)
}

async fn serve() -> Result<(), String> {
    let bind: SocketAddr = env("ALARMANT_BIND")
        .ok_or("ALARMANT_BIND is not set")?
        .parse()
        .map_err(|_| "ALARMANT_BIND is not an address")?;
    let minutes = match env("ALARMANT_MINUTES") {
        Some(m) => m.parse().map_err(|_| "ALARMANT_MINUTES is not a number")?,
        None => EscalationConfig::with_default_minutes()
            .map_err(|e| e.to_string())?
            .minutes(),
    };
    let minutes = EscalationConfig::new(minutes)
        .map_err(|e| e.to_string())?
        .minutes();
    let (api_key, pc_key, admin_key) = (
        key("ALARMANT_API_KEY")?,
        key("ALARMANT_PC_KEY")?,
        admin_key()?,
    );
    let account_json = env("ALARMANT_SERVICE_ACCOUNT_FILE")
        .and_then(|f| std::fs::read_to_string(f).ok())
        .ok_or("ALARMANT_SERVICE_ACCOUNT_FILE is not set or not readable")?;
    let account = ServiceAccount::from_json(&account_json)
        .map_err(|_| "the service account file is not valid")?;
    let transport = HttpPushTransport::new(account).map_err(|_| "the push client failed")?;
    let dir = env("ALARMANT_DATA_DIR").ok_or("ALARMANT_DATA_DIR is not set")?;
    let disk = Disk::open(Path::new(&dir)).map_err(|e| e.to_string())?;
    let devices = Arc::new(PersistedDevices::open(&disk).map_err(|e| e.to_string())?);
    let alerts = Arc::new(PersistedAlerts::open(&disk).map_err(|e| e.to_string())?);
    let enrollments = Arc::new(PersistedEnrollments::open(&disk).map_err(|e| e.to_string())?);
    let admin_addr = admin_addr()?;
    let heartbeat_key = match env("ALARMANT_HEARTBEAT_KEY") {
        None => None,
        Some(_) => {
            let k = key("ALARMANT_HEARTBEAT_KEY")?;
            if [&api_key, &pc_key, &admin_key]
                .iter()
                .any(|o| o.same_digest(&k))
            {
                return Err("ALARMANT_HEARTBEAT_KEY must differ from the other keys".to_owned());
            }
            Some(k)
        }
    };

    let clock = system_clock();
    let escalations = PersistedEscalations::open(&disk).map_err(|e| e.to_string())?;
    let mut app = App::new(
        api_key,
        pc_key,
        Arc::clone(&devices),
        alerts.clone(),
        enrollments.clone(),
        Arc::new(transport),
        minutes,
        Arc::clone(&clock),
    )
    .with_escalations(minutes, Box::new(escalations))
    .with_alert_limits(alert_limits()?);
    if let Some(k) = heartbeat_key {
        app = app.with_watchdog(k);
    }
    let app = Arc::new(app);
    let admin_server = match &app.watch {
        Some(w) => alarmant::watchstate::watchdog_admin_routes(
            Server::new(),
            admin_key.clone(),
            Arc::clone(w),
            Arc::clone(&clock),
        ),
        None => Server::new(),
    };
    let _admin = bind_loopback(
        admin_routes(
            admin_server,
            AdminConfig {
                admin_key,
                enrollments,
                devices,
                alerts,
                clock: Arc::clone(&clock),
            },
        ),
        admin_addr,
    )
    .await
    .map_err(|_| "the admin endpoint could not start (loopback address only)")?;
    let mut tls =
        alarmant::tls::paths(env("ALARMANT_TLS_CERT_FILE"), env("ALARMANT_TLS_KEY_FILE"))?;
    if let Some(names) = env("ALARMANT_TLS_SELF_SIGNED_NAMES") {
        if tls.is_some() {
            return Err(
                "set ALARMANT_TLS_SELF_SIGNED_NAMES or the certificate and key files, not both"
                    .to_owned(),
            );
        }
        tls = Some(alarmant::tls::self_signed_in(Path::new(&dir), &names)?);
    }
    let mut public = public_server(&app);
    if let Some((cert, key)) = &tls {
        public = public.tls(alarmant::tls::acceptor_from_files(cert, key)?);
        if let Ok(f) = std::fs::read(cert)
            .map_err(|_| ())
            .and_then(|c| alarmant::tls::fingerprint_hex(&c).map_err(|_| ()))
        {
            eprintln!("alarmant: serving HTTPS, certificate SHA-256 {f}");
        }
    }
    let _public = public
        .bind(bind.to_string().as_str())
        .await
        .map_err(|_| "the public address could not be bound")?;
    eprintln!("alarmant: serving; escalation step {minutes} min");
    loop {
        tokio::time::sleep(TICK).await;
        for (_, r) in tick_once(&app, clock()).await {
            if let Err(e) = r {
                eprintln!("alarmant: second contact push failed: {e:?}");
            }
        }
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("serve") if args.len() == 1 => serve().await,
        Some("enroll" | "revoke") => client(&args).await,
        Some("watchdog") => pause(&args).await,
        _ => Err("usage: alarmant serve | enroll|revoke --contact first|second | watchdog pause --minutes N".to_owned()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(m) => {
            eprintln!("alarmant: {m}");
            ExitCode::FAILURE
        }
    }
}
