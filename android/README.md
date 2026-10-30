# AlarmAnt Android app (A3)

**Builds** (`:app:assembleDebug`, 2026-10-30) with JDK 17 (Temurin 17.0.20.1), Gradle 9.8.2 wrapper, AGP 9.5.0, compileSdk 37. **Never run** on a device or emulator.

Remaining gaps:
- `app/google-services.json` (the Firebase project) is CireSnave's and is git-ignored; the plugin fails without it. A placeholder file is enough to build, not to receive pushes.
- `targetSdk` stays 36 (raising it changes runtime behaviour; do that deliberately).
- Device registration and the ack base URL/token (`config` preferences) wait on the A2 ack-auth decision.
- Build limits: `gradle.properties` caps workers at 2; run one build at a time. `local.properties` (SDK path) is git-ignored.

Design: `DESIGN-A3.md`. Flow: `AlarmMessagingService` (data push) -> `AlarmService` (alarm stream, max volume, looping, full-screen notification) -> `AlarmActivity` (Acknowledge is the only exit) -> `AckWorker` (WorkManager retry).
