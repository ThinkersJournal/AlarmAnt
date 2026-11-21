//! Structural guard for PATENT-SWEEP constraints 13-22 (CireSnave ruling 407-A, 2026-11-21): the phone
//! sends nothing but Acknowledge (plus enrolment and token upkeep, which carry no status), the PC sends
//! only Acknowledge, and the escalator takes no input but a raise, an Acknowledge and the timer.
//! This reads source files by fixed paths. It fails if a new outgoing call, route or escalator input
//! appears: the author then re-reads the claims (PATENT-MASIMO-19-662313.md) before widening a list.

use std::fs;
use std::path::PathBuf;

fn path(rel: &str) -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), rel].iter().collect()
}

fn read(rel: &str) -> String {
    let p = path(rel);
    fs::read_to_string(&p)
        .unwrap_or_else(|e| panic!("{}: {e}", p.display()))
        .replace("\r\n", "\n")
}

fn count(hay: &str, needle: &str) -> usize {
    hay.matches(needle).count()
}

const APP: &str = "android/app/src/main/kotlin/com/thinkersjournal/alarmant";

/// Every Kotlin file of the app, by name.
fn kotlin_files() -> Vec<(String, String)> {
    let mut out: Vec<_> = fs::read_dir(path(APP))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "kt"))
        .map(|p| {
            (
                p.file_name().unwrap().to_string_lossy().into_owned(),
                fs::read_to_string(&p).unwrap(),
            )
        })
        .collect();
    out.sort();
    assert!(
        out.len() >= 5,
        "the app's sources were found ({})",
        out.len()
    );
    out
}

#[test]
fn the_phone_makes_only_the_known_calls() {
    for (name, src) in kotlin_files() {
        let calls = count(&src, "requestMethod");
        let allowed = match name.as_str() {
            "AckWorker.kt" => 1,
            "Registration.kt" => 2,
            _ => 0,
        };
        assert_eq!(
            calls, allowed,
            "{name}: an outgoing HTTP call was added or removed; re-read PATENT-SWEEP 13-22 first"
        );
    }
    let ack = read(&format!("{APP}/AckWorker.kt"));
    assert!(
        ack.contains("\"POST\"") && ack.contains("/ack/"),
        "the phone's one status is Acknowledge"
    );
    let reg = read(&format!("{APP}/Registration.kt"));
    assert!(reg.contains("\"PUT\"") && reg.contains("\"POST\""));
    for p in ["/register", "/device/token"] {
        assert!(reg.contains(p), "{p}");
    }
    assert_eq!(
        count(&reg, "JSONObject()"),
        2,
        "request bodies carry the FCM token only"
    );
    assert!(!reg.contains(".put(\"status\"") && !reg.contains(".put(\"state\""));
}

#[test]
fn the_pc_program_sends_only_acknowledge() {
    let src = read("src/pcclient.rs");
    assert_eq!(count(&src, ".post("), 1, "one outgoing POST");
    assert!(src.contains("/pc/ack/"));
    for verb in [".put(", ".patch(", ".delete("] {
        assert_eq!(count(&src, verb), 0, "{verb}");
    }
}

#[test]
fn the_escalator_takes_only_a_raise_an_ack_and_the_timer() {
    let src = read("src/escalate.rs");
    let start = src.find("impl Escalator").expect("impl Escalator");
    let end = src[start..].find("\n}\n").map(|i| start + i).unwrap();
    let mut methods: Vec<&str> = src[start..end]
        .lines()
        .filter_map(|l| l.trim_start().strip_prefix("pub fn "))
        .map(|l| l.split(['(', '<']).next().unwrap())
        .collect();
    methods.sort();
    // inputs: register (a raise), ack, tick (the timer); the rest only construct or read
    assert_eq!(
        methods,
        [
            "ack",
            "at_stage",
            "new",
            "next_due",
            "register",
            "state",
            "tick",
            "with_store"
        ],
        "a new Escalator method is a new input; re-read PATENT-SWEEP 13-22 first"
    );
}

#[test]
fn the_server_has_no_route_that_takes_a_phone_or_pc_status() {
    let mut routes = Vec::new();
    for f in ["src/serve.rs", "src/routes.rs", "src/channel.rs"] {
        for l in read(f).lines() {
            let l = l.trim();
            for verb in ["post", "put", "get"] {
                if let Some(r) = l.strip_prefix(&format!("server.{verb}(\"")) {
                    let p = r.split('"').next().unwrap();
                    routes.push(format!("{} {p}", verb.to_uppercase()));
                }
            }
            if let Some(r) = l.strip_prefix("server.route(Method::") {
                let (verb, rest) = r.split_once(", \"").unwrap();
                routes.push(format!("{verb} {}", rest.split('"').next().unwrap()));
            }
        }
    }
    routes.sort();
    routes.dedup();
    let known = [
        "GET /alert/:alert_id",
        "GET /pc/alarm",
        "POST /ack/:alert_id",
        "POST /alert",
        "POST /pc/ack/:alert_id",
        "POST /register",
        "PUT /device/token",
    ];
    assert_eq!(
        routes, known,
        "a public route was added or removed; re-read PATENT-SWEEP 13-22 first"
    );
}
