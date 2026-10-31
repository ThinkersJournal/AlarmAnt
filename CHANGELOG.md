# Changelog

## 0.0.23
- A3/A8: instrumented tests for the alarm path (6 tests, pass on API 36); fix: ACTION_STOP used stopSelf(), which dropped a START queued behind it and crashed with ForegroundServiceDidNotStartInTime (now stopSelf(startId)). CI jobs `android` and `android-instrumented`.

## 0.0.22
- A3: Android lint run on main (25 warnings -> 9, 0 errors); fixes and dispositions in android/README "Lint". KTX prefs/Uri helpers, backup rules, API 26 lock-screen flags, InlinedApi suppressions with reasons.

## 0.0.21
- A3: AlarmManager watchdog. While an alarm is unacknowledged, a 5 s exact alarm restarts the service if its process died (silent gap about 30 s -> 2 to 7 s). New permission SCHEDULE_EXACT_ALARM, new Setup step.

## 0.0.20
- A3: AlarmActivity follows Do Not Disturb live (Total Silence warning appears or clears while the alarm sounds). Emulator rows: fresh-install kill, Recents swipe measured. Docs and activity only.

## 0.0.19
- A3: Total Silence warning in SetupActivity and AlarmActivity (the platform mutes the alarm there and no app can bypass it).

## 0.0.18
- A3: process-death recovery. AlarmService returns START_REDELIVER_INTENT, so the system restarts a killed alarm until Acknowledge (STOP stays NOT_STICKY).

## 0.0.17
- A3: alarms survive a reboot. ActiveAlarm persists the sounding alert until Acknowledge; BootReceiver restarts it (falls back to a full-screen notification if Android refuses the foreground start).

## 0.0.16
- A3: emulator runs for DND, process death, reboot and battery whitelist; findings in android/README. Docs only.

## 0.0.15
- A3: first run on an emulator (API 36 google_apis x86_64, WHPX): foreground alarm service, max-volume alarm-stream playback, full-screen AlarmActivity on a locked screen, back blocked, Acknowledge stops audio/service and restores volume. Docs only.

## 0.0.14
- HANDOFF rewrite (docs only).

## 0.0.13
- A3: android/README build, lint and unit-test commands and the toolchain versions used; HANDOFF rewrite. Docs only.

## 0.0.12
- Patent sweep: the three published applications (CA 2838076, US2024/0277934, US20160086473) read from Google Patents; all clear, two new design constraints (9: second notification never to a medical device; 10: one device per recipient). CA 2838076 claims are summarised, not verbatim. Docs only.

## 0.0.11
- A3: first unit tests (JUnit, no device): the alert de-duplication window (accept, reject repeat, evict oldest at 200). The window logic is now a pure function. No Rust change.

## 0.0.10
- A3: fixes from the first lint run. Foreground start now works on API 26-28 (ServiceCompat); setup walkthrough no longer reports Notifications/Full-screen as MISSING on releases that grant them at install; alert de-duplication evicts the oldest ids (StringSet was unordered); a repeat alarm start no longer leaks a wake lock; alarm tone falls back to ringtone then notification tone instead of staying silent. `lintDebug` passes with no errors. No Rust change.

## 0.0.9
- A3: first build of android/ (debug APK assembles with JDK 17, Gradle 9.8.2, AGP 9.5.0, compileSdk 37). Added the Gradle wrapper (checksum-pinned); compileSdk 36 -> 37 as the dependencies require. Still unrun on a device or emulator. No Rust change.

## 0.0.8
- A3: Android project skeleton under android/ (service, alarm playback, full-screen activity, ack worker, setup walkthrough) as UNBUILT files; excluded from the crate package. No Rust change.

## 0.0.7
- A3 design (docs only): Android alarm app flow, setup walkthrough, emulator tests and the real-phone list; no code.

## 0.0.6
- A5: escalation state machine (sent, escalated, acked, unresolved) with an AlertChannel trait and fake-channel tests. No timers, HTTP, retries or provider code. Breaking (pre-1.0): first public API.

## 0.0.5
- A5 design (docs only): escalation to a second person, meets sweep constraints 1-8; no code.

## 0.0.4
- Patent sweep: USPTO reads of every remaining granted hit (14 patents); verdicts hold; three new design constraints. Docs only.

## 0.0.3
- Patent sweep: USPTO official-record reads for 11 more granted hits (second batch); 5 hits and 5 independents still unread, sweep stays INCOMPLETE. Docs only.

## 0.0.2
- A2 design (docs only): names the shared Ants and a stub for each; no sender code.

## 0.0.1
- Patent sweep, two passes (ROADMAP A1): 22 hits, independent claims read; INCOMPLETE, so A5 stays blocked. Docs only.

## 0.0.0
- Reserved name; charter, roadmap and patent record only.
