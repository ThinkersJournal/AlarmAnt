# HANDOFF: AlarmAnt

State: lane `ciresnave-bot`; model Sonnet. FULL SPEED (read `C:\Projects\THROTTLE.md`). Ant-first (PM 2026-10-28). No force-push (PM 2026-10-29). Questions/blocks go to the PM, never the user's chat (CireSnave 2026-10-30).
Before first crates.io publish: PATENT-SWEEP.md lists CA 2838076 as summarised not verbatim, and no statuses are confirmed on the official record; read CA 2838076 claims on CIPO if the PM wants that closed.

TASK 1: PR `a1-apps` (worktree `C:\Projects\AlarmAnt-wt-sweep`, 0.0.12) in PM gate: applications read, constraints 9-10. After merge: remove worktree, prune, delete remote branch via `gh api -X DELETE`.
WAITING on CireSnave (PM has it on the board): user-level emulator system image; do NOT install until he says. Then run the real alarm path.
Next without a device: docs (README run/test commands), further pure-logic extraction. A2 ack-auth/device registration open.
Lint tip: lint caches stale results; `clean` first. Do not create local.properties (use ANDROID_HOME). Test: `gradlew :app:testDebugUnitTest`.
Parked: SMS/voice provider and SMSKit adapter (requirements in DESIGN-A5.md).
