# HANDOFF: AlarmAnt

Current state: `alarmant` 0.0.0 published on crates.io (name reserved). No code. Lane is `ciresnave-bot`; model Sonnet.
Throttle: read `C:\Projects\THROTTLE.md` at session start (FULL SPEED since 2026-10-28T04:30Z).

TASK 1 (A1, in gate): PR #1 from branch `a1-patent-sweep` (worktree `C:\Projects\AlarmAnt-wt-a1`) holds the first-pass sweep, 13 hits, status INCOMPLETE. Check it with `gh pr list`; the PM gates and merges. Remove the worktree and branch after merge (portfolio CLAUDE.md section 3).
TASK 2 (A1 completion, next): PATENT-SWEEP.md second pass is done (22 hits, US9479387B2 read). Left: read the 'not read' list in its Needs-review section; USPTO full-text, Espacenet, WIPO; the Google Patents xhr/query route gave 503 after a few calls (retry later, slowly). Claims via `python -I C:\Projects	ools\patent-claims.py <number>`. Mark complete only when all four ideas are searched on those routes, or ask the PM to accept web-search plus Google Patents as sufficient.
TASK 3 (now allowed): A2 sender, then A3 Android app (needs a real phone: ask the PM to ask CireSnave for the device). A5 stays blocked until A1 is complete. Build with CARGO_BUILD_JOBS=2, low Gradle workers, one build at a time.
Design constraints from the sweep are in PATENT-SWEEP.md (per-app override, no per-sender+window flag, no provider performance profiles, one alert priority).
