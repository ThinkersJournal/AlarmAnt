# HANDOFF: AlarmAnt

STATE (2026-11-06): FULL SPEED. PM task (board 362 ruled "A"): build A5 escalation as DESIGN-A5.md, PR per slice, [READY] each, [ASK] to stop. Main (0.0.34) has A2 `fcm`+`auth`, A4 `ack`, A9 `raise`, A6 `selftest`, A5 state machine + `escalate` timer driver + persistence store + `wire` (raise/ack hooks, EscalationConfig) + restart tests, and the Android app. A5 slices that need no provider are done; slice 3 waits on board 389.

RULES (PM, verbatim intent): phone never reports "received"/"viewed", only Acknowledge; no Decline control; escalation on a fixed timer only. Any change needs a fresh patent reading and the PM's OK. Keep PATENT-A5-READING.md current, labelled a model reading, not legal advice. The PM tries the USPTO continuity lookup for the Masimo family before any public release. Sweep stays incomplete: say "outside as read", never "clear".
Secrets: real `android/app/google-services.json` git-ignored; FCM tokens never logged; service-account key is board 354 (CireSnave, VPS only).

NEXT A5: slice 3 `AlertChannel` adapter behind the trait: provider pick is CireSnave's, ask the PM first, wire none until told. Slice 4 ConfigurAnt/PersistAnt backends for `EscalationConfig` and `EscalationStore` (need the Ants) plus a scheduler loop calling `Escalator::tick`.
Then: A2 slice 2b (HTTPAnt `PushTransport`, check `deny.toml` licences), A6 scheduler loop, Kotlin `AckWorker` stop on 401/404.

Tools: `C:\Projects\tools\patent-claims.py <numbers>`; emulator AVD `alarm36` in `C:\Users\cires\lane-tools\alarmant`.
Rules: Sonnet only, questions to the PM, bump the version per PR (next 0.0.36), `Cargo.lock` must match (CI --locked). Primary checkout stays on `main`; branches in worktrees `C:\Projects\AlarmAnt-wt-<topic>`.

WAIT: board 389 ruled by CireSnave (SMS/voice provider; 0 hits means it left the open list, then read the ruling) | check: grep -c '^### 389\.' /c/Projects/CIRESNAVE-DECISIONS.md | wake: 2026-11-07T04:00:00Z
