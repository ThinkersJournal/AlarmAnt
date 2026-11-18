# HANDOFF: AlarmAnt

STATE (2026-11-18): FULL SPEED. Main (0.2.7+) has the whole server side of ruling 389: escalation chain first app -> second app -> PC alarm (`escalate`, `wire`, `channel`, `second`), persisted stores (`persist`), loopback admin endpoint + `enroll`/`revoke` client (`adminapi`, `admin`), the `alarmant` binary (`serve`, `enroll|revoke --contact first|second`), the PC alarm program `alarmant-pc` (Windows, Core Audio unmute + max volume, siren, Stop dialog = Acknowledge; `pcclient`, `POST /pc/ack/:id`), the Android app, and the patent readings. Nothing has been run live: needs a Firebase service account (board 354, CireSnave).

RULES (PM): phone never reports "received"/"viewed", only Acknowledge; no Decline or negative status (PATENT-SWEEP constraints 13-22: server never records a phone or PC fetch; PC sends only Acknowledge, no status); escalation on a fixed timer only; any change needs a fresh patent reading and the PM's OK. Say "outside as read", never "clear" (sweep incomplete; not legal advice). Secrets never in repo, argv or logs.
Workflow: Sonnet only; one changelog FRAGMENT per PR (`changelog.d/<topic>.md`, `kind: ...`, `---`); never edit CHANGELOG.md, Cargo.toml version or Cargo.lock (a new dependency's lock entry is the one exception). Sign PR bodies `— lane: alarmant`. Primary stays on `main`; branches in `C:\Projects\AlarmAnt-wt-<topic>`; after a squash-merge use a merge commit, never force-push. Messages: compact format, `key=` pairs only. Clean worktree+branch after each merge.

IN FLIGHT:
- Branch `a8-release-signing` (worktree `AlarmAnt-wt-a8`, pushed, NO PR yet): `signingConfig` from env, `.github/workflows/android-release.yml` (tag `android-v*` only), `ANDROID-RELEASE.md`, one fragment. Unrun (no JDK; the first tag push with CireSnave's secrets is the real test). Open ONE PR for it on the next day's slot (today's #69 is merged; PR for 11-17 used). Slice (h) is complete on main (#68, #69).

NEXT, in order:
1. Open the A8 PR (above) when the day's slot allows; send [READY]; then ask CireSnave (via the PM) to set the five repo secrets listed in ANDROID-RELEASE.md.
2. Phone-session readiness: cleartext allowance or https for a LAN server; board 354 files (CireSnave).
3. Known gap: re-enrolment does not void a replaced phone's in-flight alerts (revoke first). PC alarm board and second-push queue are in memory: an alert already at the PC step is not raised again after a restart.
4. Owed: a person's confirmation of the three patent readings (outside as read); the sweep is incomplete.

Tools: `C:\Projects\tools\patent-claims.py <numbers>`; emulator AVD `alarm36` in `C:\Users\cires\lane-tools\alarmant`. Windows: `bash tools/...` is not available, use `python tools/check-fragment.py origin/main`.
