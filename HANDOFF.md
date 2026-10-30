# HANDOFF: AlarmAnt

State: lane `ciresnave-bot`; model Sonnet. FULL SPEED (read `C:\Projects\THROTTLE.md`). Ant-first (PM 2026-10-28). No force-push (PM 2026-10-29). Questions/blocks go to the PM, never the user's chat (CireSnave 2026-10-30).
Before first crates.io publish: read the 3 unread applications (CA 2838076, US2024/0277934, US20160086473) or list them "not read" with reason in PATENT-SWEEP.md.

Android toolchain INSTALLED (CireSnave accepted the SDK licence in his own session 2026-10-30), user-level at `C:\Users\cires\lane-tools\alarmant` (jdk, sdk, gradle, dl); nothing on shared PATH. Build: set JAVA_HOME=...\jdk\jdk-17.0.20.1+1, ANDROID_HOME=...\sdk, then `android\gradlew.bat --no-daemon :app:assembleDebug` (needs a placeholder git-ignored `app/google-services.json`).
TASK 1: PR `a3-build` (worktree `C:\Projects\AlarmAnt-wt-a3build`, 0.0.9) in PM gate: wrapper + compileSdk 37, debug APK builds. After merge: remove worktree, prune, delete remote branches `a3-build` and stale `handoff-a3-install` via `gh api -X DELETE`.
TASK 2: A3 code PR: FCM service + alarm playback fixes (kotlin warning in AlarmMessagingService.kt:25); then lint/unit tests. A2 ack-auth/device registration open.
Parked: SMS/voice provider and SMSKit adapter (requirements in DESIGN-A5.md).
