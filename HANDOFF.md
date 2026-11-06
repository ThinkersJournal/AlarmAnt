# HANDOFF: AlarmAnt

STATE (2026-11-06): FULL SPEED. PM task (board 362 ruled "A"): build A5 escalation as DESIGN-A5.md, PR per slice, [READY] each, [ASK] to stop. Main (0.0.30) has A2 `fcm`+`auth`, A4 `ack`, A9 `raise`, A6 `selftest`, the A5 pure state machine, the Android app. Slice 1 `escalate` (Escalator timer driver, 0.0.31) is on branch `a5-escalator`: check `gh pr list`; if merged, start slice 2.

RULES (PM, verbatim intent): phone never reports "received"/"viewed", only Acknowledge; no Decline control; escalation on a fixed timer only. Any change needs a fresh patent reading and the PM's OK. Keep PATENT-A5-READING.md current, labelled a model reading, not legal advice. The PM tries the USPTO continuity lookup for the Masimo family before any public release. Sweep stays incomplete: say "outside as read", never "clear".
Secrets: real `android/app/google-services.json` git-ignored; FCM tokens never logged; service-account key is board 354 (CireSnave, VPS only).

NEXT A5 slices: 2 persist `Alert` stage/due (PersistAnt `AlertStore`) so a restart resumes timers; 3 `AlertChannel` adapter behind the trait (provider pick is CireSnave's; ask the PM first, wire none until told); 4 ConfigurAnt `EscalationConfig { minutes, person2 }`, person 2's number out of logs; 5 hook `Escalator` into `raise` (register) and `ack` (cancel).
Then: A2 slice 2b (HTTPAnt `PushTransport`, check `deny.toml` licences), A6 scheduler loop, Kotlin `AckWorker` stop on 401/404.

Tools: `C:\Projects\tools\patent-claims.py <numbers>`; emulator AVD `alarm36` in `C:\Users\cires\lane-tools\alarmant`.
Rules: Sonnet only, questions to the PM, bump the version per PR (next 0.0.32), `Cargo.lock` must match (CI --locked).
