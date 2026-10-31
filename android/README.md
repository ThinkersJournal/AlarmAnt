# AlarmAnt Android app (A3)

**Builds** (`:app:assembleDebug`, 2026-10-30) with JDK 17 (Temurin 17.0.20.1), Gradle 9.8.2 wrapper, AGP 9.5.0, compileSdk 37. **Run on an emulator 2026-10-30** (see "Emulator run"); never on a physical device, no real FCM push.

Remaining gaps:
- `app/google-services.json` (the Firebase project) is CireSnave's and is git-ignored; the plugin fails without it. A placeholder file is enough to build, not to receive pushes.
- `targetSdk` stays 36 (raising it changes runtime behaviour; do that deliberately).
- Device registration and the ack base URL/token (`config` preferences) wait on the A2 ack-auth decision.
- Build limits: `gradle.properties` caps workers at 2; run one build at a time. `local.properties` (SDK path) is git-ignored.

## Build, lint, test

Needs JDK 17 and an Android SDK (platform 37.0, build-tools 37.0.0). Set `JAVA_HOME` and `ANDROID_HOME`; do not create `local.properties` (lint flags its Windows path form and caches the result). Copy or create `app/google-services.json` (git-ignored; a placeholder builds, only the real file receives pushes).

    gradlew.bat --no-daemon :app:assembleDebug
    gradlew.bat --no-daemon clean :app:lintDebug      # clean first: lint caches stale results
    gradlew.bat --no-daemon :app:testDebugUnitTest

One build at a time. Unit tests cover pure logic only (`AlertLog.window`); the alarm path itself needs a device or emulator and has not been run.

Design: `DESIGN-A3.md`. Flow: `AlarmMessagingService` (data push) -> `AlarmService` (alarm stream, max volume, looping, full-screen notification) -> `AlarmActivity` (Acknowledge is the only exit) -> `AckWorker` (WorkManager retry).

## Emulator run (2026-10-30, CireSnave: "Yes, install the emulator image for AlarmAnt.")

Package ids (SDK manager, under `lane-tools\alarmant\sdk`): `emulator` 37.2.13, `system-images;android-36;google_apis;x86_64` rev 7 (4.3 GB unpacked; emulator 1.1 GB). AVD `alarm36`, made without `-d` (`avdmanager -d pixel_6` fails on the image's `devices.xml`); point `ANDROID_AVD_HOME` at a lane-tools folder. WHPX works with no admin step. Cold boot 215 s: `emulator -avd alarm36 -no-window -no-audio -no-snapshot -gpu swiftshader_indirect -memory 2048`.

Needs a placeholder `app/google-services.json` (package `com.thinkersjournal.alarmant`). The shell can start the non-exported service only after `adb root` (google_apis images allow it). Use `MSYS_NO_PATHCONV=1` for `/sdcard` paths; `adb shell` re-splits extras containing spaces.

Measured: START intent -> service foreground (mediaPlayback), MediaPlayer `USAGE_ALARM` started, alarm volume 6 -> 7 (max); locked screen -> `AlarmActivity` on top; Back ignored; Acknowledge -> 0 alarm players, 0 foreground services, volume back to 6, activity gone, ack work enqueued (its network constraint not exercised).

Not covered: real FCM delivery, DND bypass (no policy grant), audible output (`-no-audio`), battery-optimisation exemption, a physical device, process death and reboot. Finding: `STOP` sent via `startForegroundService` crashes (`ForegroundServiceDidNotStartInTimeException`); the app uses `startService`, so only external callers hit it.

### Emulator run 2 (2026-10-30; waits of 12 s needed: cold process start with Firebase init takes about 6 s)

- **DND, no policy grant:** Priority-only: alarm player started, `mutedState:none`, stream not muted. Total Silence (`zen_mode=2`): player reports started but `mutedState:opPlayAudio`, alarm stream `Muted: true`. **The alarm is silent under Total Silence** (a platform rule apps cannot bypass); Alarms-only was not run. The channel shows `mBypassDnd=false` without the policy grant. Decision for CireSnave/PM: tell users to avoid Total Silence, or detect `INTERRUPTION_FILTER_NONE` and say so in SetupActivity.
- **Process death:** `kill -9` of the app process while alarming: 0 players, 0 foreground services after 6 s. `START_NOT_STICKY` means a crash or OOM kill silences an active alarm and nothing restarts it. Row: consider `START_REDELIVER_INTENT`, or re-raise from the ack-pending state.
- **Reboot:** fixed in 0.0.17 (`ActiveAlarm` + `BootReceiver`); before it, no alarm survived a reboot. Measured on API 36: alarm sounding, `adb reboot`, receiver ran at BOOT_COMPLETED (about 90 s after `sys.boot_completed`) and the alarm sounded again (1 player, 1 foreground service, "Background started FGS: Allowed"). Not measured: the notification fallback (Android allowed the start here), Acknowledge after a recovered alarm (the shell lost `adb root` over the reboot), a locked-credential boot.
- **Battery whitelist:** `cmd deviceidle whitelist +pkg` works from `adb root`; that only shows the whitelist is accepted. The in-app request flow was not driven.
