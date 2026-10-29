# HANDOFF: AlarmAnt

State: main has A1 sweep, DESIGN-A2.md; `alarmant` 0.0.3 on main, no code. Lane `ciresnave-bot`; model Sonnet. FULL SPEED (read `C:\Projects\THROTTLE.md`).
Rule: ant-first (PM 2026-10-28): no new hand-rolled config/secrets/redaction/audit/HTTP/retry/scheduling/alert-delivery/release code without PM ack. A2 design-only until the PM answers.

TASK 1: PR for branch `a1-ppubs-complete` (worktree `C:\Projects\AlarmAnt-wt-a1c`) in gate (version 0.0.4): all granted hits now read on the USPTO record. After merge: remove worktree, prune, delete remote branch via `gh api -X DELETE`. The PM decides whether the sweep may be marked COMPLETE (Espacenet, WIPO, assignee searches not run) and so whether A5 unblocks.
TASK 2: if the PM accepts: A5 design (escalation) under constraints 1-8 in PATENT-SWEEP.md, design-only per ant-first. Otherwise run Espacenet/WIPO/assignee searches.
TASK 3: A3 Android app needs a real phone: `[ASK]` to the PM when starting. Build limits: CARGO_BUILD_JOBS=2, low Gradle workers.
