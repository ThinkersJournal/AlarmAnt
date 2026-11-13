# HANDOFF: AlarmAnt

STATE (2026-11-13): FULL SPEED. Main has A2 sender (`fcm`, `auth`, `push`), A4 `ack` (digest-only `AckToken`), A9 `raise`, A6 `selftest`, A5 state machine + `escalate` + store + `wire`, `routes` (`/ack`, `/alert`, `/register`), registration slices a-c (enrolment code, `POST /register`), the Android app, and the A5/PC-alarm patent readings. Open PR: #53 (slice d, `PUT /device/token`), queued with the PM.

RULES (PM): phone never reports "received"/"viewed", only Acknowledge; no Decline or any negative status (PATENT-SWEEP constraints 13-22: the server never acts on or records a phone or PC fetch; PC sends only Acknowledge); escalation on a fixed timer only; any change needs a fresh patent reading and the PM's OK. Say "outside as read", never "clear" (sweep incomplete; not legal advice). Secrets: real `android/app/google-services.json` git-ignored; FCM tokens, codes, keys never logged; service-account key is board 354 (CireSnave, VPS only).
Workflow: Sonnet only; changelog FRAGMENTS (one `changelog.d/<topic>.md`, first line `kind: ...`, then `---`); never edit CHANGELOG.md, Cargo.toml version or Cargo.lock (the gate stamps). Sign every PR comment/body `— lane: alarmant`. Primary checkout stays on `main`; branches in `C:\Projects\AlarmAnt-wt-<topic>`; stacked PRs: `rebase --onto origin/main <old parent tip>` (squash merges), retarget with `gh pr edit --base main`, [READY] again. Messages: compact format, header words with `key=` pairs only.

RULING 389 (CireSnave 2026-11-12): app alarm to first contact, app alarm to second contact, PC alarm if neither is acknowledged; no SMS/voice until money. See DESIGN-A5.md "Ruling 389", PATENT-PC-ALARM-READING.md, PATENT-MASIMO-19-662313.md (all outside as read).

NEXT, in order:
1. Registration slices (e) revoke (server-side record delete + clear in-flight ack digests; no restore, no self-invalidation, no roll-back), (f) CLI `alarmant enroll --contact first|second` / `revoke`, (g) PersistAnt backends, (h) Android registration step. Each: known-bad test first, one fragment, retarget to main as parents merge.
2. Three-channel step 1: registry holds two recipients (`first`, `second`), each own ack token; second app push on the timer; either ack ends the alert. Then step 3: the PC alarm (Windows user-session program, Core Audio unmute + max volume + siren, polls the server; limits are in DESIGN-A5).
3. A8 signing: `signingConfig` from env in `android/app/build.gradle.kts` + release workflow (tag `android-v*`, never on PRs) + `ANDROID-RELEASE.md`; uncommitted draft in worktree `AlarmAnt-wt-a8`, untested: no JDK on this box (set JAVA_HOME/ANDROID_HOME or verify in CI only). Android build/lint/emulator jobs already exist and pass.
4. Phone session readiness (answered to PM 2026-11-12): needs a runnable server binary (none: the crate is a library), enroll CLI, app registration step, cleartext allowance or https for a LAN server, board 354 files.
A6 remainder: scheduler loop and notifier need a service binary. Tools: `C:\Projects\tools\patent-claims.py <numbers>`; emulator AVD `alarm36` in `C:\Users\cires\lane-tools\alarmant`.

WAIT: #53 merged by the PM gate | check: gh pr view 53 --repo ThinkersJournal/AlarmAnt --json state --jq .state | wake: 2026-11-13T15:00:00Z
