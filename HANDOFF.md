# HANDOFF: AlarmAnt

STATE (2026-11-20, after #72 = 0.3.0): FULL SPEED. Main (0.2.7+) has the whole server side of ruling 389: escalation chain first app -> second app -> PC alarm (`escalate`, `wire`, `channel`, `second`), persisted stores (`persist`), loopback admin endpoint + `enroll`/`revoke` client (`adminapi`, `admin`), the `alarmant` binary (`serve`, `enroll|revoke --contact first|second`), the PC alarm program `alarmant-pc` (Windows, Core Audio unmute + max volume, siren, Stop dialog = Acknowledge; `pcclient`, `POST /pc/ack/:id`), the Android app, and the patent readings. Nothing has been run live: needs a Firebase service account (board 354, CireSnave).

RULES (PM): phone never reports "received"/"viewed", only Acknowledge; no Decline or negative status (PATENT-SWEEP constraints 13-22: server never records a phone or PC fetch; PC sends only Acknowledge, no status); escalation on a fixed timer only; any change needs a fresh patent reading and the PM's OK. Say "outside as read", never "clear" (sweep incomplete; not legal advice). Secrets never in repo, argv or logs.
Workflow: Sonnet only; one changelog FRAGMENT per PR (`changelog.d/<topic>.md`, `kind: ...`, `---`); never edit CHANGELOG.md, Cargo.toml version or Cargo.lock (a new dependency's lock entry is the one exception). Sign PR bodies `— lane: alarmant`. Primary stays on `main`; branches in `C:\Projects\AlarmAnt-wt-<topic>`; after a squash-merge use a merge commit, never force-push. Messages: compact format, `key=` pairs only. Clean worktree+branch after each merge.

IN FLIGHT:
- #72 MERGED (0.3.0, 00:43Z): re-enrolment voids old ack tokens, https-only enrolment, server HTTPS (`ALARMANT_TLS_*`, `ALARMANT_TLS_SELF_SIGNED_NAMES`), Android `PinnedTls`, PC alarm trust (`ALARMANT_PC_SERVER_CERT_FILE`). Follow-ups from its reviews: a handshake test of `PinTrustManager`/hostname verifier (needs a JDK keystore), a Windows owner-only ACL for `tls-key.pem` (ROADMAP A11, A12 on the note branch).
- Branch `alert-rate-limit` (worktree `AlarmAnt-wt-ratelimit`, no PR yet): ROADMAP A10, `POST /alert` per-key rate limit (module `ratelimit`, defaults 6/h, 20/day, `ALARMANT_RATE_PER_HOUR`/`_DAY`); red-first test seen failing, sabotage fails 5 tests, 206 tests, fragment `changelog.d/alert-rate-limit.md` (BREAKING for a sender over 6/h). PR day: 2026-11-22 UTC (11-21 is the note's); [READY] to pm-assist cc pm-ants.
- PR #73 (branch `synapse-channel-note`, worktree `AlarmAnt-wt-synnote`): docs-only `docs/SYNAPSE-ADAPTER.md`, opened 2026-11-21, queued by pm-assist as 0.3.1 (gate pushes the version to the branch: PULL FIRST). After merge: delete branch local+remote (read back 404), `git worktree remove ../AlarmAnt-wt-synnote`. No adapter/watchdog code before CireSnave reads the note.
- A8 (#70) merged: first `android-v*` tag push waits for the PM's word and CireSnave's five secrets (board 462).

WAIT: A10 + apache-text-fix + PATENT-SYNAPSE-READING PR day (open ONE PR from `alert-rate-limit`, BREAKING, [READY] to pm-assist cc pm-ants) | check: gh pr list --head alert-rate-limit --state all | wake: 2026-11-22T00:10:00Z

NEXT, in order:
1. 11-21 UTC: open the note PR; run and record the patent reading (Part: inbound channel and heartbeat watchdog).
2. 11-22 UTC: open the `alert-rate-limit` PR.
3. Add ROADMAP rows A11 (PinTrustManager handshake test) and A12 (Windows key ACL) on the note branch.
4. Phone-session readiness needs a real device (board 354 Firebase, CireSnave).
5. Owed: a person's confirmation of the patent readings (outside as read); the sweep is incomplete.

Tools: `C:\Projects\tools\patent-claims.py <numbers>`; emulator AVD `alarm36` in `C:\Users\cires\lane-tools\alarmant`. Windows: use `python tools/check-fragment.py origin/main`; run `cargo fmt --check` before every push (CI fails on it). Many files here are CRLF: edit with a line-ending-aware script (`grep -c $'\r'` lies on this box).
