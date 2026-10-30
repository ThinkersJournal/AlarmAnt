# AlarmAnt Android app (A3)

**Builds** (`:app:assembleDebug`, 2026-10-30) with JDK 17 (Temurin 17.0.20.1), Gradle 9.8.2 wrapper, AGP 9.5.0, compileSdk 37. **Never run** on a device or emulator.

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
