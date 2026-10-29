# HANDOFF: AlarmAnt

State: lane `ciresnave-bot`; model Sonnet. FULL SPEED (read `C:\Projects\THROTTLE.md`). Ant-first (PM 2026-10-28): no hand-rolled config/secrets/redaction/audit/HTTP/retry/scheduling/alert-delivery code without PM ack.
Sweep ACCEPTED sufficient to build A5 code (PM 2026-10-29). Before first crates.io publish: read the 3 unread applications (CA 2838076, US2024/0277934, US20160086473) or list them "not read" with reason in PATENT-SWEEP.md. Espacenet/WIPO/assignee searches stay recorded read-again triggers.

TASK 1: PR #5 (`a5-design`, worktree `C:\Projects\AlarmAnt-wt-a5`, 0.0.5) in PM gate. After merge: remove worktree, prune, delete remote branch via `gh api -X DELETE`. No force-push (PM prefers a plain merge of main).
TASK 2: after #5 merges, new worktree: A5 state machine (sent, escalated, acked, unresolved) + AlertChannel trait + fake-channel tests; no HTTP, timer or retry code.
TASK 3: provider pick parked by PM until CireSnave decides SMSKit (A5 uses own AlertChannel trait; no provider code).
TASK 4: A3 Android app needs a real phone: `[ASK]` to the PM when starting. CARGO_BUILD_JOBS=2, low Gradle workers.
All questions/blocks go to the PM (CireSnave standing order 2026-10-29).
