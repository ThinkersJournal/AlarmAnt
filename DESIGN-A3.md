# A3 design: Android alarm app

Status: DESIGN ONLY. No Android toolchain on this box (java, gradle, adb, emulator, SDK all absent, checked 2026-10-29); installing one waits on the PM. Emulator-first; tests that need a real phone are listed at the end (PM 2026-10-29).
Constraints: PATENT-SWEEP.md 1 (override per app, always on while installed), 2 (no emergency-code or location reply), 4 (one priority), 5 (no wearable or gesture acknowledge), 8 (alert is a data push, nothing parses text).

## Flow
1. A high-priority FCM data message arrives (A2: priority HIGH, data only, fields `alert_id`, `title`, `body`).
2. `FirebaseMessagingService.onMessageReceived` starts a foreground service (the high-priority FCM exemption allows this from the background).
3. The service posts a notification on a channel with importance HIGH, category ALARM, a full-screen intent, and bypass-DND set; takes a wake lock; sets the alarm stream to maximum; plays a looping sound with `AudioAttributes.USAGE_ALARM`.
4. The full-screen activity shows the alert text and one control: Acknowledge. Nothing else stops the sound (no swipe-away, no gesture, no wearable action).
5. Acknowledge stops the sound, restores the previous alarm volume, and queues the call back to `POST /ack/{alert_id}` (A4) with the per-device token. The queue retries until it succeeds, so a dead network does not leave the alarm running or lose the ack.
6. Message handling is idempotent on `alert_id`: a repeat push re-arms nothing already acknowledged.

## Setup walkthrough (one time, shown on first run and on the self-test failing)
- Notification policy access (Do Not Disturb): `ACTION_NOTIFICATION_POLICY_ACCESS_SETTINGS`; the app never turns itself off, so the override is per app and always on (constraint 1).
- Battery optimisation exemption: `ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS` (allowed for a sideloaded app; ROADMAP A7).
- Full-screen intent permission (`USE_FULL_SCREEN_INTENT`): on Android 14 and later the user may have to enable it in settings.
- Notification permission (`POST_NOTIFICATIONS`, Android 13+).
- The walkthrough re-reads each grant at every launch and shows what is missing; a missing grant is the main silent-failure cause (ROADMAP risks), so A6 reports it.

## Choices
- Kotlin, current stable Android Gradle Plugin and AndroidX at build time (latest-versions rule); minSdk to be set from the oldest phone CireSnave cares about, default 26.
- Retry of the ack call uses the platform's WorkManager, not hand-written retry code (ant-first spirit; the Rust Ants do not apply on Android).
- No Play Store: sideload build and install notes are A7, signing is A8.
- FCM only (A2); the app holds no secret except its own device token.

## Emulator tests (no phone)
Foreground service starts from a data push; alarm stream goes to max and is restored; sound loops until Acknowledge; ack call queued while offline then sent; duplicate `alert_id` ignored; permission walkthrough shows each missing grant.

## Real-phone list (for CireSnave; not a blocker)
- Sound actually audible on the alarm stream with the ringer silent and DND on (priority-only and total silence).
- Full-screen alert over a locked, sleeping screen.
- Delivery after hours in Doze and after the app is swiped away, on CireSnave's own phone make.
- Vendor background killers and autostart lists (differ per OEM).
- FCM high-priority delivery latency.
