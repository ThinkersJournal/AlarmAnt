# HANDOFF: AlarmAnt

Current state: `alarmant` 0.0.0 published on crates.io (name reserved). No code. Lane is `ciresnave-bot`; model Sonnet.
Throttle: read `C:\Projects\THROTTLE.md` at session start (THROTTLED on 2026-10-26 until ~2026-10-28T01:00Z: A1 only, no A2/A3, no sub-agents, header-line messages).

TASK 1 (A1, in gate): PR from branch `a1-patent-sweep` (worktree `C:\Projects\AlarmAnt-wt-a1`) holds the first-pass sweep, 13 hits, status INCOMPLETE. Check it with `gh pr list`; the PM gates and merges. Remove the worktree and branch after merge (portfolio CLAUDE.md section 3).
TASK 2 (A1 completion, next): find the granted patent for US20130346521A1 and read its claims; USPTO full-text scan, Espacenet and WIPO; assignee searches (PagerDuty, Atlassian, Splunk, Pushover, Apple, Google). Claims via `python -I C:\Projects\tools\patent-claims.py <number>`. Mark the sweep complete only when all four ideas are searched on those routes.
TASK 3 (after FULL SPEED): A2 sender, then A3 Android app (needs a real phone: ask the PM to ask CireSnave for the device). A5 stays blocked until A1 is complete. Build with CARGO_BUILD_JOBS=2, low Gradle workers, one build at a time.
Design constraints from the sweep are in PATENT-SWEEP.md (per-app override, no per-sender+window flag, no provider performance profiles, one alert priority).
