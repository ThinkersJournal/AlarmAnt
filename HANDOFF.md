# HANDOFF: AlarmAnt

STATE (2026-11-11): FULL SPEED. Main (0.0.42) has A2 `fcm`+`auth`+`push` (HttpPushTransport over HTTPAnt; base-URL setter test-only), A4 `ack`, A9 `raise`, A6 `selftest` (persisted state, next wake, run_step, notifier trait), A5 state machine + `escalate` + persistence store + `wire`, and the Android app. A5 slices needing no provider are done; slice 3 waits on board 389.

RULES (PM, verbatim intent): phone never reports "received"/"viewed", only Acknowledge; no Decline control; escalation on a fixed timer only. Any change needs a fresh patent reading and the PM's OK. Keep PATENT-A5-READING.md current, labelled a model reading, not legal advice. The PM tries the USPTO continuity lookup for the Masimo family before any public release. Sweep stays incomplete: say "outside as read", never "clear".
Secrets: real `android/app/google-services.json` git-ignored; FCM tokens never logged; service-account key is board 354 (CireSnave, VPS only).

NEXT A5: slice 3 `AlertChannel` adapter behind the trait: provider pick is CireSnave's, ask the PM first, wire none until told. Slice 4 ConfigurAnt/PersistAnt backends for `EscalationConfig` and `EscalationStore` (need the Ants) plus a scheduler loop calling `Escalator::tick`.
A6 `run_loop` + `Schedule` is DONE (0.0.41, runtime-agnostic, trait and fake only; the service must supply a tokio sleep and a real FailureNotifier, channel is board 131/389, CireSnave's). Kotlin `AckWorker` stop on 401/404 is DONE (0.0.42, #42). Nothing unblocked remains: all else waits on board 389 or PM tasks below. A2 2b is DONE (#38, #39). No open PRs.

Tools: `C:\Projects\tools\patent-claims.py <numbers>`; emulator AVD `alarm36` in `C:\Users\cires\lane-tools\alarmant`.
Rules: Sonnet only, questions to the PM, bump the version per PR (next 0.0.44), `Cargo.lock` must match (CI --locked). Primary checkout stays on `main`; branches in worktrees `C:\Projects\AlarmAnt-wt-<topic>`.

WAIT: #45 merged; then merge main into a7-sideload-notes (keep 0.0.46), push, [READY] | check: gh pr view 45 --repo ThinkersJournal/AlarmAnt --json state --jq .state | wake: 2026-11-11T23:30:00Z
WAIT: board 389 ruled by CireSnave (SMS/voice provider; 0 hits means it left the open list, then read the ruling) | check: grep -c '^### 389\.' /c/Projects/CIRESNAVE-DECISIONS.md | wake: 2026-11-12T00:00:00Z

OPEN PM TASKS (2026-11-11): (1) read US 19/662,313 (continuation of 19/020,852, filed 2026-04-29, pending): USPTO ODP API needs a key (401), PPUBS session endpoint guessed wrong (404), Google finds no 2026 publication, Justia 403; say plainly if unpublished, map to A5 as in PATENT-A5-READING.md; (2) A6 remainder (scheduler loop; ask PM on notifier); (3) DESIGN-A5 "Question 2026-11-10" (person 2 via the app) awaits CireSnave's ruling on board 389.
