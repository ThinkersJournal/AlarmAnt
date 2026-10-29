# HANDOFF: AlarmAnt

State: lane `ciresnave-bot`; model Sonnet. FULL SPEED (read `C:\Projects\THROTTLE.md`). Ant-first (PM 2026-10-28): no hand-rolled config/secrets/redaction/audit/HTTP/retry/scheduling/alert-delivery code in Rust without PM ack. No force-push (PM 2026-10-29).
Sweep ACCEPTED sufficient to build A5 code (PM 2026-10-29). Before first crates.io publish: read the 3 unread applications (CA 2838076, US2024/0277934, US20160086473) or list them "not read" with reason in PATENT-SWEEP.md.
main has: A1 sweep, DESIGN-A2, DESIGN-A5, A5 state machine + AlertChannel (0.0.6).

TASK 1: PR for `a3-design` (worktree `C:\Projects\AlarmAnt-wt-a3`, 0.0.7) in PM gate: DESIGN-A3.md. After merge: remove worktree, prune, delete remote branch via `gh api -X DELETE`.
TASK 2: A3 build is blocked on an Android toolchain (none on the box); [ASK] sent to PM 2026-10-29 to install JDK17 + cmdline-tools + SDK + emulator. Emulator-first; real-phone tests are the list in DESIGN-A3.md (PM asks CireSnave). Limits: CARGO_BUILD_JOBS=2, low Gradle workers, one build at a time.
Parked: SMS/voice provider and SMSKit adapter (CireSnave deciding SMSKit's future; requirements in DESIGN-A5.md).
All questions/blocks go to the PM (CireSnave standing order 2026-10-29).
