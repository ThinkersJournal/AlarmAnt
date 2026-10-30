# HANDOFF: AlarmAnt

State: lane `ciresnave-bot`; model Sonnet. FULL SPEED (read `C:\Projects\THROTTLE.md`). Ant-first (PM 2026-10-28). No force-push (PM 2026-10-29). Questions/blocks go to the PM, never the user's chat (CireSnave 2026-10-30).
Before first crates.io publish: read the 3 unread applications (CA 2838076, US2024/0277934, US20160086473) or list them "not read" with reason in PATENT-SWEEP.md.

Android toolchain INSTALLED (CireSnave accepted the SDK licence in his own session 2026-10-30), user-level at `C:\Users\cires\lane-tools\alarmant` (jdk, sdk, gradle, dl); nothing on shared PATH. Build: set JAVA_HOME=...\jdk\jdk-17.0.20.1+1, ANDROID_HOME=...\sdk, then `android\gradlew.bat --no-daemon :app:assembleDebug` (needs a placeholder git-ignored `app/google-services.json`).
TASK 1: PR `a3-tests` (worktree `C:\Projects\AlarmAnt-wt-a3tests`, 0.0.11) in PM gate: AlertLog unit tests. After merge: remove worktree, prune, delete remote branch via `gh api -X DELETE`.
WAITING on CireSnave (PM has it on the board): user-level emulator system image; do NOT install until he says. Then run the real alarm path.
Next without a device: docs (README run/test commands), further pure-logic extraction. A2 ack-auth/device registration open.
Lint tip: lint caches stale results; `clean` first. Do not create local.properties (use ANDROID_HOME). Test: `gradlew :app:testDebugUnitTest`.
Parked: SMS/voice provider and SMSKit adapter (requirements in DESIGN-A5.md).
