# AlarmAnt roadmap

Ruled by CireSnave 2026-10-26 (decision 287 S15 c, A): build it as its own project; power saving disabled for this app only. Stopgap until it exists: Pushover emergency priority, with its emergency channel allowed to override Do Not Disturb.

Decision (PM 2026-11-13, option a): the server is the single writer. `alarmant enroll`/`revoke` are a thin client to a loopback admin endpoint (binds 127.0.0.1 only; admin key from an env var or a user-only file, never argv, logs or error text; constant-time compare; revoke atomic under the server lock; a wrong or missing key mutates nothing).

| Row | What | Status | Owner / blocker |
|---|---|---|---|
| A1 | Patent sweep (escalation policies are crowded; on-call vendors hold patents) BEFORE A5 is built | open | lane, first |
| A2 | Sender: a Cloudflare Worker (or a script on the VPS) that sends a high-priority push through Google's push service; an acknowledge endpoint | open | lane |
| A3 | Android app: foreground alarm on the alarm volume stream at maximum, full-screen alert, repeats until Acknowledge, one-time Do-Not-Disturb permission and battery-optimisation exemption walkthrough | open | lane; needs a real phone |
| A4 | Acknowledge call back to the sender; per-alert state | handler + in-memory store built (0.0.25); HTTP route built (0.0.44); PersistAnt store, token issue open | lane |
| A5 | Escalation: unacknowledged after N minutes tries a second person by text or call (a few cents each); design after A1 | open | blocked on A1 |
| A6 | Daily self-test alarm that proves the whole chain; a missed ack the next morning is the failure signal | logic built (0.0.28); scheduler loop, persisted state, failure notifier open | lane |
| A7 | Sideload (no Play Store fee) build and install notes | written (`android/SIDELOAD.md`, 0.0.46) | lane |
| A8 | CI for the Android build, signed release artefact policy | open | lane; CireSnave for the signing key |
| A9 | API for other projects to raise an alert (one small, documented call) | handler + docs built (0.0.27); HTTP route and key config built (0.0.45); device registration open (proposal: DESIGN-REGISTRATION.md) | lane |
| A10 | **Defect, opened 2026-11-20 (pm-ants): `POST /alert` has no rate or count limit** (read in `raise.rs`, `serve.rs`, `routes.rs`; the only limiter in the repository is the failed-attempt one on `/register`), so any holder of an API key can raise alerts without bound. Fix, independent of the Synapse adapter and in the next code PR after #72: a per-key cap (proposal 1 open, 6 an hour, 20 a day; overall 3 open) with a fixed 429 and no push sent; red-first test: the N+1th alert inside the window is refused, and a second key held by the same sender cannot bypass it (the limit is keyed on the key's owner, one row per owner in config) | open | lane; numbers for CireSnave (via pm-ants) |
| A11 | A handshake test of Android's `PinTrustManager` and the default hostname verifier (right certificate accepted, wrong one refused, hostname mismatch refused); today only the fingerprint check and the socket-factory wiring are unit-tested, and the Kotlin tests only run on CI. Needs a JDK keystore in the test (left open by the #72 reviews, 2026-11-20) | open | lane |
| A12 | Owner-only ACL for `tls-key.pem` on Windows (the key is created owner-only on Unix only; on Windows it inherits the data directory's permissions, which the README tells the operator to keep private). Left open by the #72 reviews, 2026-11-20 | open | lane |

Known risks: Android background killers and Do-Not-Disturb rules change every year; the self-test is the answer to silent failure.
