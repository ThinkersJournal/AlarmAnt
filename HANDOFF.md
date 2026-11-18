# HANDOFF: AlarmAnt

STATE (2026-11-17): FULL SPEED. Main (0.2.7+) has the whole server side of ruling 389: escalation chain first app -> second app -> PC alarm (`escalate`, `wire`, `channel`, `second`), persisted stores (`persist`), loopback admin endpoint + `enroll`/`revoke` client (`adminapi`, `admin`), the `alarmant` binary (`serve`, `enroll|revoke --contact first|second`), the PC alarm program `alarmant-pc` (Windows, Core Audio unmute + max volume, siren, Stop dialog = Acknowledge; `pcclient`, `POST /pc/ack/:id`), the Android app, and the patent readings. Nothing has been run live: needs a Firebase service account (board 354, CireSnave).

RULES (PM): phone never reports "received"/"viewed", only Acknowledge; no Decline or negative status (PATENT-SWEEP constraints 13-22: server never records a phone or PC fetch; PC sends only Acknowledge, no status); escalation on a fixed timer only; any change needs a fresh patent reading and the PM's OK. Say "outside as read", never "clear" (sweep incomplete; not legal advice). Secrets never in repo, argv or logs.
Workflow: Sonnet only; one changelog FRAGMENT per PR (`changelog.d/<topic>.md`, `kind: ...`, `---`); never edit CHANGELOG.md, Cargo.toml version or Cargo.lock (a new dependency's lock entry is the one exception). Sign PR bodies `— lane: alarmant`. Primary stays on `main`; branches in `C:\Projects\AlarmAnt-wt-<topic>`; after a squash-merge use a merge commit, never force-push. Messages: compact format, `key=` pairs only. Clean worktree+branch after each merge.

IN FLIGHT:
- PR #69 (branch `token-rotation`, worktree `AlarmAnt-wt-tok`): slice (h) part 2, `PUT /device/token` on FCM rotation (`TokenWorker`), plus this HANDOFF; main already has #68 (registration step) and #67. Unbuilt locally; CI is the check. Wait for the PM gate, then remove the worktree and branch. One PR per day (CLAUDE.md 2026-11-16).
- Branch `a8-release-signing` (worktree `AlarmAnt-wt-a8`): WIP pushed, untested (no JDK locally): `signingConfig` from env vars in `android/app/build.gradle.kts`. Still to do: release workflow (tag `android-v*`, never on PRs) and `ANDROID-RELEASE.md`; verify in CI only.

NEXT, in order:
1. Slice (h) is done once #69 merges. Next: finish A8 signing (below).
2. Finish A8 signing (above).
3. Phone-session readiness: cleartext allowance or https for a LAN server; board 354 files (CireSnave).
4. Known gap: re-enrolment does not void a replaced phone's in-flight alerts (revoke first). PC alarm board and second-push queue are in memory: an alert already at the PC step is not raised again after a restart.
5. Owed: a person's confirmation of the three patent readings (outside as read); the sweep is incomplete.

Tools: `C:\Projects\tools\patent-claims.py <numbers>`; emulator AVD `alarm36` in `C:\Users\cires\lane-tools\alarmant`. Windows: `bash tools/...` is not available, use `python tools/check-fragment.py origin/main`.

WAIT: #69 merged by the PM gate | check: gh pr view 69 --repo ThinkersJournal/AlarmAnt --json state --jq .state | wake: 2026-11-18T00:22:00Z
