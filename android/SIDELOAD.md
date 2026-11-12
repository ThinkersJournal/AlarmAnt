# Sideloading the AlarmAnt app (A7)

AlarmAnt is not on the Play Store (no developer fee, no review), so the phone gets the APK directly. State at 0.0.46: you can build and install the **debug** APK; a **signed release** APK waits on A8 (the signing key is CireSnave's). The app cannot receive real pushes until the sender has a real FCM key (board 354) and the phone's registration exists (see "Not yet possible").

## What you need

- JDK 17 and the Android SDK (platform 37.0, build-tools 37.0.0); `JAVA_HOME` and `ANDROID_HOME` set. Details and the emulator setup: `android/README.md`.
- `android/app/google-services.json` for the real Firebase project (CireSnave's; git-ignored, never commit it). A placeholder builds but receives no pushes.
- A phone on Android 8.0 or newer (`minSdk` 26), with USB debugging on, and `adb` on this computer.

## Build

    cd android
    gradlew.bat --no-daemon :app:assembleDebug

The APK is `android/app/build/outputs/apk/debug/app-debug.apk` (that path exists on this box after a build). A debug APK is signed with the local debug key, so it is fine for CireSnave's own phone and not for anyone else's.

## Install

With the phone connected (accept the "Allow USB debugging" prompt on it):

    adb devices                      # the phone must be listed as "device"
    adb install -r app/build/outputs/apk/debug/app-debug.apk

`-r` keeps the app's data on an update. If the phone already holds a build signed with another key, the install fails with `INSTALL_FAILED_UPDATE_INCOMPATIBLE`: uninstall first (`adb uninstall com.thinkersjournal.alarmant`), which erases the app's saved state.

Without a cable: copy the APK to the phone, open it from the Files app, and allow "Install unknown apps" for that app when Android asks. The setting is per app and can be switched off again afterwards.

## First run: the setup screen

Open AlarmAnt once. `SetupActivity` lists each grant as OK or MISSING with a Fix button. Do all of them; an alarm that Android may silence or delay is the failure this app exists to prevent:

1. **Notifications** (runtime permission on Android 13 and newer).
2. **Do Not Disturb access** (the policy access the app may request; it does not override Total Silence, below).
3. **Full-screen alerts** (the Acknowledge screen over the lock screen; user-revocable on Android 14 and newer).
4. **Exact alarms** (lets the watchdog restart a killed alarm in seconds, not minutes).
5. **Battery optimisation off** (sideloaded apps may ask for this exemption; the Play Store restricts it).

Two things the app cannot fix, both shown as a WARNING on the setup and alarm screens: **Total Silence** Do Not Disturb mode silences the alarm stream and no app can bypass it; and some phone makers' own battery managers kill background apps beyond the settings above (check the maker's "protected apps" or "autostart" list).

## Updating

Build again and `adb install -r` over the old one. Version name and code are in `android/app/build.gradle.kts`.

## Not yet possible

- **Registration:** the phone's FCM token and ack token reach the sender by a path that is not built (DESIGN-A2, "registration ... still open"). Until it exists nothing tells the sender which phone to alert.
- **Real pushes:** need the Firebase service-account key on the sender (board 354, CireSnave, VPS only).
- **Release signing:** A8. Do not hand a debug APK to anyone else.
- **Not tested here:** a physical phone, any OEM battery manager, an Android 8 or 9 device. The app has run on the API 36 emulator only (`android/README.md`).
