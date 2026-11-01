# HANDOFF: AlarmAnt

STATE (2026-11-01): soft-stopped by PRIORITY mode (THROTTLE.md); the PM restarts this lane when work returns. Main has A2 `fcm`, A4 `ack`, A9 `raise`, A6 `selftest` (0.0.28), the A5 pure state machine (0.0.6, stays, sends nothing), and the Android app with CI. In flight at stop: #29 (patent pass 3 + PATENT-A5-READING.md, 0.0.29) and #30 (A2 slice 2a `auth`, 0.0.30, includes this HANDOFF). The PM's sweep merges green heads: read the forge (`gh pr view 29 30`); if #30 conflicts (CHANGELOG, Cargo.toml/lock) rebase it on main.

HOLDS (do not break):
- A5 is BLOCKED by board 362 (Masimo family; CireSnave to confirm PATENT-A5-READING.md). Build nothing more on A5, wire no provider. The phone never reports "received" or "viewed" and has no Decline control without a fresh reading of the Masimo family (DESIGN-A5.md rule).
- The patent sweep is INCOMPLETE; wording "outside as read", never "clear"; not legal advice. USPTO continuity for the Masimo family is not reachable by this lane.
- The real `android/app/google-services.json` is git-ignored; never commit. FCM tokens are never logged. The service-account key file lives on the VPS only (board 354, owed by CireSnave, not yet requested).

NEXT, in order (none touches escalation):
1. A2 slice 2b: HTTPAnt (`httpant-client` 0.5.22, async-only) `PushTransport` against a local fake server; check what it adds to `deny.toml` licences first. Then slice 3: ConfigurAnt config (`fcm.service_account_file`, project id, API key via env), PersistAnt `AlertStore`, HTTP routes `POST /alert` and `POST /ack/{id}`, device registration (issue the ack token, store the FCM token).
2. A6 rest: scheduler loop around `SelfTest::tick`, persist `SelfTest`, failure notifier (operator email, not the phone; the channel choice is tied to A5, so ask before choosing).
3. Kotlin `AckWorker` should stop retrying on 401/404; placeholder icon; A7 sideload notes; targetSdk 37 trial with an emulator run; misplaced POST_NOTIFICATIONS comment in SetupActivity.kt.
Needs from CireSnave (board 354): service-account key (later), one phone session, release signing key. A3's last row, A7's proof and A8's signed artefact wait on those.

Tools: `C:\Projects\tools\patent-claims.py <numbers>` prints claims verbatim; web search with `allowed_domains: [patents.google.com]` reaches patent pages. Emulator: AVD `alarm36` in `C:\Users\cires\lane-tools\alarmant` (adb root for non-exported services; about 215 s to boot, wait about 90 s more, dismiss the System UI ANR; Windows paths to `adb install`). The CI instrumented job is not branch-protection-required; its one flake (AlarmPathTest destroyed-activity race) was fixed in #27.
Rules: Sonnet only, questions to the PM, bump the version per PR (next 0.0.31), `Cargo.lock` must match the version (CI runs --locked).
