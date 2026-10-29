# HANDOFF: AlarmAnt

State: lane `ciresnave-bot`; model Sonnet. FULL SPEED (read `C:\Projects\THROTTLE.md`). Ant-first (PM 2026-10-28): no hand-rolled config/secrets/redaction/audit/HTTP/retry/scheduling/alert-delivery code without PM ack. No force-push (PM 2026-10-29).
Sweep ACCEPTED sufficient to build A5 code (PM 2026-10-29). Before first crates.io publish: read the 3 unread applications (CA 2838076, US2024/0277934, US20160086473) or list them "not read" with reason in PATENT-SWEEP.md.

TASK 1: PR for `a5-state-machine` (worktree `C:\Projects\AlarmAnt-wt-a5code`, 0.0.6) in PM gate: A5 state machine + AlertChannel trait + fake-channel tests. After merge: remove worktree, prune, delete remote branch via `gh api -X DELETE`.
TASK 2: A3 Android app needs a real phone: `[ASK]` to the PM when starting. CARGO_BUILD_JOBS=2, low Gradle workers.
Parked: SMS/voice provider and SMSKit adapter (CireSnave deciding SMSKit's future; adapter requirements in DESIGN-A5.md).
All questions/blocks go to the PM (CireSnave standing order 2026-10-29).
