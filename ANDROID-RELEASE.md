# Android release signing (A8)

Release signing reads the keystore from the environment, so no secret is in the repository.
`android/app/build.gradle.kts` signs `release` only when `ALARMANT_KEYSTORE_FILE` is set; without it
`assembleRelease` makes an unsigned APK (what a PR build or a local check gets).

## What the workflow does

`.github/workflows/android-release.yml` runs only on a pushed tag `android-v*` (never on a PR or a
branch push). It writes the keystore and the real `google-services.json` from repository secrets, runs
`:app:assembleRelease`, checks the APK is signed with `apksigner verify`, and keeps it as the workflow
artifact `alarmant-release-apk`. It publishes nothing outward.

## Dry run (no real key)

`Actions > Android release > Run workflow` (input `dry_run`, default true) runs the separate `dry-run` job: it makes a
throwaway PKCS12 keystore with `keytool` and a random password (masked; in the job's environment only), writes the same
placeholder `google-services.json` that `ci.yml` uses, builds `:app:assembleRelease` through the same Gradle signing path,
runs `apksigner verify`, and compares the APK's certificate SHA-256 with the throwaway key's. The APK is kept one day as
`alarmant-dryrun-apk-NOT-A-RELEASE`. It creates no release and no tag and uses no repository secret. It proves the build and
signing wiring only: not the real keystore, not the real Firebase file, not a phone install. The tag path (`release` job) runs
only on a pushed `android-v*` tag and is not exercised by a dry run. A dry-run APK is never given to anyone.

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

## Verifying a release end to end

Key material is supplied only through `with-secret` (names only here; no value is in a file, argv, log or this repository).
1. The run for the tag is green, and its `Check the APK is signed` step passed (`apksigner verify` on the one APK; an `unsigned` name fails it).
2. Download `alarmant-release-apk`; run `apksigner verify --print-certs` on it and compare the certificate SHA-256 with the one recorded for the upload key (kept with the keystore, not here).
3. Install it on the phone, finish Setup (every row OK, including Registered with sender), and let the daily self-test (A6) raise an alert; Acknowledge it. That is the proof the release build receives a push and acknowledges.
4. A tag push from a fork or without the secrets fails at the first step with a named missing secret, never with a silently unsigned APK.

Status: written without a JDK on the authoring box and not yet run. The first tag push is the test;
until then the signing path is unverified (the PR build checks only that the unsigned build still works).
