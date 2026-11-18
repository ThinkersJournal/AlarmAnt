kind: Android release signing and release workflow
---
`assembleRelease` signs from `ALARMANT_KEYSTORE_*` environment variables when set (unsigned otherwise), and `.github/workflows/android-release.yml` builds and verifies a signed APK only on a pushed `android-v*` tag, keeping it as a workflow artifact. `ANDROID-RELEASE.md` lists the secrets and steps. Unverified: nothing has been run with a real keystore; the PR build proves only that the unsigned build still works.
