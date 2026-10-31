# AlarmAnt roadmap

Ruled by CireSnave 2026-10-26 (decision 287 S15 c, A): build it as its own project; power saving disabled for this app only. Stopgap until it exists: Pushover emergency priority, with its emergency channel allowed to override Do Not Disturb.

| Row | What | Status | Owner / blocker |
|---|---|---|---|
| A1 | Patent sweep (escalation policies are crowded; on-call vendors hold patents) BEFORE A5 is built | open | lane, first |
| A2 | Sender: a Cloudflare Worker (or a script on the VPS) that sends a high-priority push through Google's push service; an acknowledge endpoint | open | lane |
| A3 | Android app: foreground alarm on the alarm volume stream at maximum, full-screen alert, repeats until Acknowledge, one-time Do-Not-Disturb permission and battery-optimisation exemption walkthrough | open | lane; needs a real phone |
| A4 | Acknowledge call back to the sender; per-alert state | handler + in-memory store built (0.0.25); HTTP route, PersistAnt store, token issue open | lane |
| A5 | Escalation: unacknowledged after N minutes tries a second person by text or call (a few cents each); design after A1 | open | blocked on A1 |
| A6 | Daily self-test alarm that proves the whole chain; a missed ack the next morning is the failure signal | open | lane |
| A7 | Sideload (no Play Store fee) build and install notes | open | lane |
| A8 | CI for the Android build, signed release artefact policy | open | lane; CireSnave for the signing key |
| A9 | API for other projects to raise an alert (one small, documented call) | open | lane |

Known risks: Android background killers and Do-Not-Disturb rules change every year; the self-test is the answer to silent failure.
