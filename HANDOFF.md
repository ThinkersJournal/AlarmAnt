# HANDOFF: AlarmAnt

STATE (2026-11-18): FULL SPEED. Main (0.2.7+) has the whole server side of ruling 389: escalation chain first app -> second app -> PC alarm (`escalate`, `wire`, `channel`, `second`), persisted stores (`persist`), loopback admin endpoint + `enroll`/`revoke` client (`adminapi`, `admin`), the `alarmant` binary (`serve`, `enroll|revoke --contact first|second`), the PC alarm program `alarmant-pc` (Windows, Core Audio unmute + max volume, siren, Stop dialog = Acknowledge; `pcclient`, `POST /pc/ack/:id`), the Android app, and the patent readings. Nothing has been run live: needs a Firebase service account (board 354, CireSnave).

RULES (PM): phone never reports "received"/"viewed", only Acknowledge; no Decline or negative status (PATENT-SWEEP constraints 13-22: server never records a phone or PC fetch; PC sends only Acknowledge, no status); escalation on a fixed timer only; any change needs a fresh patent reading and the PM's OK. Say "outside as read", never "clear" (sweep incomplete; not legal advice). Secrets never in repo, argv or logs.
Workflow: Sonnet only; one changelog FRAGMENT per PR (`changelog.d/<topic>.md`, `kind: ...`, `---`); never edit CHANGELOG.md, Cargo.toml version or Cargo.lock (a new dependency's lock entry is the one exception). Sign PR bodies `— lane: alarmant`. Primary stays on `main`; branches in `C:\Projects\AlarmAnt-wt-<topic>`; after a squash-merge use a merge commit, never force-push. Messages: compact format, `key=` pairs only. Clean worktree+branch after each merge.

IN FLIGHT:
- PR #71 (branch `pc-board-persist`, worktree `AlarmAnt-wt-pcpersist`): persisted escalation records + PC board rebuilt on restart. A8 (#70) is merged: the first `android-v*` tag push waits for the PM's word and CireSnave's five secrets (board 462). After #71 merges remove the worktree and branch (local + remote, read back 404).

NEXT, in order:
1. A8 live test: when the PM says so (and CireSnave has set the five secrets, board 462), push the first `android-v*` tag and verify per ANDROID-RELEASE.md. Until then nothing to do on A8.
2. Phone-session readiness: cleartext allowance or https for a LAN server; board 354 files (CireSnave).
3. Known gap: re-enrolment does not void a replaced phone's in-flight alerts (revoke first). (The restart gap is closed by the `pc-board-persist` PR: escalation records persist and the PC board is rebuilt.)
4. Owed: a person's confirmation of the three patent readings (outside as read); the sweep is incomplete.

Tools: `C:\Projects\tools\patent-claims.py <numbers>`; emulator AVD `alarm36` in `C:\Users\cires\lane-tools\alarmant`. Windows: `bash tools/...` is not available, use `python tools/check-fragment.py origin/main`.

WAIT: #71 merged by the PM gate | check: gh pr view 71 --repo ThinkersJournal/AlarmAnt --json state --jq .state | wake: 2026-11-19T18:03:00Z
