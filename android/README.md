# AlarmAnt Android app (A3), UNBUILT skeleton

Written as files only (PM 2026-10-29): no JDK, Gradle, Android SDK or emulator has been installed, so **nothing here has been compiled or run**. Treat every file as unverified until the first build.

Known gaps to settle at first build:
- No Gradle wrapper yet (needs a Gradle install to generate it).
- AGP 9.5.0 / Gradle 9.x are assumed to bring built-in Kotlin support; if not, add the Kotlin Android plugin. Versions were the latest stable on Maven Google/Central on 2026-10-29 (AGP 9.5.0, firebase-bom 35.1.0, core-ktx 1.19.1, work 2.12.0, activity 1.13.0, google-services 4.5.0). `compileSdk`/`targetSdk` 36 must be raised to the newest platform installed.
- `app/google-services.json` (the Firebase project) is CireSnave's and is git-ignored.
- Device registration and the ack base URL/token (`config` preferences) wait on the A2 ack-auth decision.
- Build limits: `gradle.properties` caps workers at 2; run one build at a time.

Design: `DESIGN-A3.md`. Flow: `AlarmMessagingService` (data push) -> `AlarmService` (alarm stream, max volume, looping, full-screen notification) -> `AlarmActivity` (Acknowledge is the only exit) -> `AckWorker` (WorkManager retry).
