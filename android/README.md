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

- **DND, no policy grant:** Priority-only: alarm player started, `mutedState:none`, stream not muted. Total Silence (`zen_mode=2`): player reports started but `mutedState:opPlayAudio`, alarm stream `Muted: true`. **The alarm is silent under Total Silence** (a platform rule apps cannot bypass); Alarms-only was not run. The channel shows `mBypassDnd=false` without the policy grant. Handled in 0.0.19 by warning, not bypassing (impossible): `TotalSilence.isOn` (reads `currentInterruptionFilter`, needs no grant) puts a WARNING line in SetupActivity and on AlarmActivity. Measured on API 36 with relaunch after each `set_dnd`: priority -> no warning, none -> warning, alarms -> no warning; AlarmActivity shows the warning under none. 0.0.20: AlarmActivity follows the filter live (`ACTION_INTERRUPTION_FILTER_CHANGED`); measured with the alarm sounding: off -> no warning, none -> warning, alarms -> cleared.
- **Process death:** `kill -9` of the app process while alarming: 0 players, 0 foreground services after 6 s. Fixed in 0.0.18 (`START_REDELIVER_INTENT`): measured on API 36, `kill -9` while alarming, the system logged "Scheduling restart of crashed service ... in 31400ms" and the alarm sounded again 31 s later (1 player). **The silent gap is the platform's crash-restart backoff** (29 to 31 s in three runs on this emulator, after repeated kills); the app cannot shorten it. Acknowledge (STOP) still ends it for good (0 players, `activealarm` prefs empty). Fresh install (uninstall, install, one alarm, one `kill -9`): restart scheduled in 29136 ms, alarm back at about 30 s, so the gap is not accumulated backoff: expect about 30 s of silence after any kill. A watchdog (exact `AlarmManager` re-check while an alarm is active) could shorten it; not built. Recents swipe: task removed from Recents (count 1 -> 0), alarm kept sounding (1 player, 1 foreground service); the Acknowledge screen is gone, so reopening it needs the notification. Not measured: a real low-memory kill (lmkd sends the same SIGKILL as `kill -9`; assumed equal).
- **Reboot:** fixed in 0.0.17 (`ActiveAlarm` + `BootReceiver`); before it, no alarm survived a reboot. Measured on API 36: alarm sounding, `adb reboot`, receiver ran at BOOT_COMPLETED (about 90 s after `sys.boot_completed`) and the alarm sounded again (1 player, 1 foreground service, "Background started FGS: Allowed"). Not measured: the notification fallback (Android allowed the start here), Acknowledge after a recovered alarm (the shell lost `adb root` over the reboot), a locked-credential boot.
- **Battery whitelist:** `cmd deviceidle whitelist +pkg` works from `adb root`; that only shows the whitelist is accepted. The in-app request flow was not driven.

## Watchdog (0.0.21)

While an alarm is unacknowledged, `AlarmWatchdog` fires a `WatchdogReceiver` every 5 s; if the process-local `AlarmService.playingId` is null (fresh process after a kill) it restarts the service, and `playingId` stops the system's own late restart from re-creating the player. Acknowledge cancels it. **Gap after `kill -9` (API 36 emulator, wait for player 0 then time to player 1):** before 29 to 31 s (4 runs); after, exact alarms granted 7 s; exact denied (inexact fallback) 2 s; exact granted with forced deep Doze (`deviceidle force-idle`, app not allowlisted) 6 s. One run each after, so read it as "2 to 7 s", not a distribution; the bound is the 5 s period plus process start. The player keeps one `piid` across watchdog ticks and across the system's late restart (no audio restart).

**Permissions and policy:** `SCHEDULE_EXACT_ALARM` (not granted by default on API 33+; SetupActivity has an "Exact alarms" step, and without it the watchdog falls back to `setAndAllowWhileIdle`, which Doze may delay by minutes on a real device: the emulator does not show that). The alternative `USE_EXACT_ALARM` is granted at install and cannot be revoked, but Google Play restricts it to apps whose core function is an alarm clock or calendar; whether AlarmAnt qualifies is CireSnave's call at Play submission and is not decided here. **Battery:** nothing is scheduled while no alarm is active; during an alarm it is one wake every 5 s (about 12 per minute) until Acknowledge, on top of the alarm's own wake lock and audio. Not measured: real battery draw, a physical device, whether `dumpsys alarm` shows the pending alert removed after Acknowledge (its Watchdog entries are history counts), OEM background-kill policies that also cancel alarms (force-stop cancels them).

## Lint (0.0.22, lint on main b417976 with AGP 9.5.0)

Ran: `clean :app:lintDebug`, `:app:testDebugUnitTest`, `:app:assembleDebug` (all pass). There is **no detekt or ktlint** in the project (no config, no plugin); none was run. Before: 0 errors, 25 warnings (9 issue types). After: **0 errors, 9 warnings**. A control: the first run listed the warnings below by name, the re-run after each edit listed the remainder by name (not just a count).

Fixed: `ApplySharedPref` x2 (deliberate `commit()`, now `edit(commit = true)`, the comment in ActiveAlarm says why), `UseKtx` x5 (`edit {}`, `toUri()`), `InlinedApi` x4 (each use is behind an SDK gate; suppressed on the two functions with the reason beside it), `UnusedAttribute` x2 (`showWhenLocked`/`turnScreenOn` need API 27; API 26 now gets the window flags in AlarmActivity; not run on an API 26 device), `DataExtractionRules` (rules and `fullBackupContent` added, both exclude everything: nothing here should be restored elsewhere).

Left, with the reason:
- `OldTargetApi` (targetSdk 36; lint's latest is 37): raising it changes runtime behaviour, so it is a deliberate task with a device run, not a lint fix.
- `BatteryLife` (REQUEST_IGNORE_BATTERY_OPTIMIZATIONS): Play restricts this permission to listed app kinds. Same class of question as USE_EXACT_ALARM: **CireSnave's call at Play submission**.
- `MissingApplicationIcon`: there is no icon asset; making one is a design task for CireSnave.
- `SetTextI18n` x6: the UI is built in code with English literals on purpose (single-user app, no `strings.xml`). Accepted until localisation is wanted.

Emulator smoke after the edits (API 36): alarm sounding, `activealarm` prefs written, `kill -9` -> back in 2 s (watchdog), Acknowledge -> 0 players and empty prefs; SetupActivity renders all steps. Not run: tapping each Fix button after the `toUri()` change, an API 26 device, the Play-policy items.
