# HANDOFF: AlarmAnt

STATE (2026-11-19): FULL SPEED. Main (0.2.7+) has the whole server side of ruling 389: escalation chain first app -> second app -> PC alarm (`escalate`, `wire`, `channel`, `second`), persisted stores (`persist`), loopback admin endpoint + `enroll`/`revoke` client (`adminapi`, `admin`), the `alarmant` binary (`serve`, `enroll|revoke --contact first|second`), the PC alarm program `alarmant-pc` (Windows, Core Audio unmute + max volume, siren, Stop dialog = Acknowledge; `pcclient`, `POST /pc/ack/:id`), the Android app, and the patent readings. Nothing has been run live: needs a Firebase service account (board 354, CireSnave).

RULES (PM): phone never reports "received"/"viewed", only Acknowledge; no Decline or negative status (PATENT-SWEEP constraints 13-22: server never records a phone or PC fetch; PC sends only Acknowledge, no status); escalation on a fixed timer only; any change needs a fresh patent reading and the PM's OK. Say "outside as read", never "clear" (sweep incomplete; not legal advice). Secrets never in repo, argv or logs.
Workflow: Sonnet only; one changelog FRAGMENT per PR (`changelog.d/<topic>.md`, `kind: ...`, `---`); never edit CHANGELOG.md, Cargo.toml version or Cargo.lock (a new dependency's lock entry is the one exception). Sign PR bodies `— lane: alarmant`. Primary stays on `main`; branches in `C:\Projects\AlarmAnt-wt-<topic>`; after a squash-merge use a merge commit, never force-push. Messages: compact format, `key=` pairs only. Clean worktree+branch after each merge.

IN FLIGHT:
- Branch `reenrol-void` (worktree `AlarmAnt-wt-reenrol`, pushed at 6246df3, NO PR): ONE fragment `changelog.d/reenrol-void.md` covers: re-enrolment voids the replaced phone's in-flight ack tokens (PM ruling 11-19 a); Android rejects `http://` links; `ALARMANT_TLS_CERT_FILE`/`_KEY_FILE` HTTPS; `ALARMANT_TLS_SELF_SIGNED_NAMES` self-signed pair + `alarmant enroll` link with `f=<sha256>` (needs `ALARMANT_PUBLIC_URL`); Android `PinnedTls` (pm-ants ruling: leaf SHA-256 pin only, default hostname verifier, one @SuppressLint, system trust without `f=`); `PATENT-TLS-PIN-READING.md` (4 patents, outside as read, constraints T1-T5); httpant 0.7.10, rcgen 0.14.10. 194 Rust tests, clippy, deny ok. Kotlin tests NEVER RUN (no JDK): say so in the PR body in those words and name `PinnedTlsTest` + the new `RegistrationTest` pin assertions as CI-only.
- PR rules: [READY] goes to `pm-assist`, cc `pm-ants`; ONE PR per UTC day (#71 used 11-19). Open it 11-20 UTC, PR body lists the patents the reading covered, then ask for a second reader on `PinnedTls` (`/code-review high`, findings answered) before merge. Keep working the branch meanwhile; after merge delete branch + worktree, start a fresh branch.
- A8 (#70) merged: first `android-v*` tag push waits for the PM's word and CireSnave's five secrets (board 462).

WAIT: reenrol-void PR day (open the PR) | check: git ls-remote --heads origin reenrol-void | wake: 2026-11-20T00:10:00Z

NEXT, in order:
1. On 11-20 UTC open the `reenrol-void` PR (above).
2. Phone-session readiness needs a real device (board 354 Firebase, CireSnave); the README LAN text assumes a phone run that has not happened.
3. Owed: a person's confirmation of the patent readings (outside as read); the sweep is incomplete.

Tools: `C:\Projects\tools\patent-claims.py <numbers>`; emulator AVD `alarm36` in `C:\Users\cires\lane-tools\alarmant`. Windows: use `python tools/check-fragment.py origin/main`. Many files here are CRLF: edit with a line-ending-aware script.
