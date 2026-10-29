# HANDOFF: AlarmAnt

State: main has A1 sweep, DESIGN-A2.md; `alarmant` 0.0.3 on main, no code. Lane `ciresnave-bot`; model Sonnet. FULL SPEED (read `C:\Projects\THROTTLE.md`).
Rule: ant-first (PM 2026-10-28): no new hand-rolled config/secrets/redaction/audit/HTTP/retry/scheduling/alert-delivery/release code without PM ack. A2 design-only until the PM answers.

TASK 1: PRs #4 (a1-ppubs-complete, 0.0.4) and the A5 design PR (branch a5-design, worktree AlarmAnt-wt-a5, 0.0.5, stacked on #4) are in gate; after merge remove worktrees, prune, delete remote branches via gh api. Rebase a5-design onto main once #4 merges.
TASK 2: if the PM accepts: A5 design (escalation) under constraints 1-8 in PATENT-SWEEP.md, design-only per ant-first. Otherwise run Espacenet/WIPO/assignee searches.
TASK 3: A3 Android app needs a real phone: `[ASK]` to the PM when starting. Build limits: CARGO_BUILD_JOBS=2, low Gradle workers.
