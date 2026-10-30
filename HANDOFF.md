# HANDOFF: AlarmAnt

State: lane `ciresnave-bot`; model Sonnet. FULL SPEED (read `C:\Projects\THROTTLE.md`). Ant-first (PM 2026-10-28). No force-push (PM 2026-10-29). Questions/blocks go to the PM, never the user's chat (CireSnave 2026-10-30).
main has: A1 sweep (applications read, constraints 1-10), DESIGN-A2/A3/A5, A5 state machine (0.0.6), android/ builds + lints + 4 unit tests (0.0.13). Nothing run on a device.
Toolchain (user-level, no shared PATH): `C:\Users\cires\lane-tools\alarmant` (jdk, sdk, gradle). Commands in android/README.md.

TASK 1: PR `a3-docs` (worktree `C:\Projects\AlarmAnt-wt-docs`, 0.0.13) in PM gate. After merge: remove worktree, prune, delete remote branch via `gh api -X DELETE`.
WAITING on CireSnave (PM has it on the board): user-level emulator system image; do NOT install until he says. Then run the real alarm path.
Without a device: more pure-logic extraction from AlarmService/AckWorker with tests. A2 ack-auth/device registration open.
Before first crates.io publish: CA 2838076 claims were summarised not verbatim, statuses unconfirmed on the official record (PATENT-SWEEP.md).
Parked: SMS/voice provider and SMSKit adapter (requirements in DESIGN-A5.md).
