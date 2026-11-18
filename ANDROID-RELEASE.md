# Android release signing (A8)

Release signing reads the keystore from the environment, so no secret is in the repository.
`android/app/build.gradle.kts` signs `release` only when `ALARMANT_KEYSTORE_FILE` is set; without it
`assembleRelease` makes an unsigned APK (what a PR build or a local check gets).

## What the workflow does

`.github/workflows/android-release.yml` runs only on a pushed tag `android-v*` (never on a PR or a
branch push). It writes the keystore and the real `google-services.json` from repository secrets, runs
`:app:assembleRelease`, checks the APK is signed with `apksigner verify`, and keeps it as the workflow
artifact `alarmant-release-apk`. It publishes nothing outward.

## Repository secrets (CireSnave sets them; never in a file, argv or a log)

| Secret | Content |
| --- | --- |
| `ALARMANT_KEYSTORE_B64` | the upload keystore, base64 of the `.jks` file |
| `ALARMANT_KEYSTORE_PASSWORD` | keystore password |
| `ALARMANT_KEY_ALIAS` | key alias |
| `ALARMANT_KEY_PASSWORD` | key password |
| `GOOGLE_SERVICES_JSON` | the owner's real Firebase `google-services.json` |

## Cutting a release

1. Tag the exact commit on `main`: `android-v<version>` (annotated), push the tag.
2. Download `alarmant-release-apk` from the run, install it on the phone.
3. Keep the keystore and its passwords somewhere that is not this repository; losing the key means a
   new key and a reinstall.

Status: written without a JDK on the authoring box and not yet run. The first tag push is the test;
until then the signing path is unverified (the PR build checks only that the unsigned build still works).
