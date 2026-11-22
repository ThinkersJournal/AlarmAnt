# HANDOFF: AlarmAnt

STATE (2026-11-20, after #72 = 0.3.0): FULL SPEED. Main (0.2.7+) has the whole server side of ruling 389: escalation chain first app -> second app -> PC alarm (`escalate`, `wire`, `channel`, `second`), persisted stores (`persist`), loopback admin endpoint + `enroll`/`revoke` client (`adminapi`, `admin`), the `alarmant` binary (`serve`, `enroll|revoke --contact first|second`), the PC alarm program `alarmant-pc` (Windows, Core Audio unmute + max volume, siren, Stop dialog = Acknowledge; `pcclient`, `POST /pc/ack/:id`), the Android app, and the patent readings. Nothing has been run live: needs a Firebase service account (board 354, CireSnave).

RULES (PM): phone never reports "received"/"viewed", only Acknowledge; no Decline or negative status (PATENT-SWEEP constraints 13-22: server never records a phone or PC fetch; PC sends only Acknowledge, no status); escalation on a fixed timer only; any change needs a fresh patent reading and the PM's OK. Say "outside as read", never "clear" (sweep incomplete; not legal advice). Secrets never in repo, argv or logs.
Workflow: Sonnet only; one changelog FRAGMENT per PR (`changelog.d/<topic>.md`, `kind: ...`, `---`); never edit CHANGELOG.md, Cargo.toml version or Cargo.lock (a new dependency's lock entry is the one exception). Sign PR bodies `— lane: alarmant`. Primary stays on `main`; branches in `C:\Projects\AlarmAnt-wt-<topic>`; after a squash-merge use a merge commit, never force-push. Messages: compact format, `key=` pairs only. Clean worktree+branch after each merge.

IN FLIGHT:
- #72 MERGED (0.3.0, 00:43Z): re-enrolment voids old ack tokens, https-only enrolment, server HTTPS (`ALARMANT_TLS_*`, `ALARMANT_TLS_SELF_SIGNED_NAMES`), Android `PinnedTls`, PC alarm trust (`ALARMANT_PC_SERVER_CERT_FILE`). Follow-ups from its reviews: a handshake test of `PinTrustManager`/hostname verifier (needs a JDK keystore), a Windows owner-only ACL for `tls-key.pem` (ROADMAP A11, A12 on the note branch).
- Branch `alert-rate-limit` (worktree `AlarmAnt-wt-ratelimit`, no PR yet; also holds `GET /alert/:id`, `tests/constraints.rs` (ruling 407-A), ROADMAP A13; rulings 478 and 407-A accepted: adapter as designed, NO watchdog code until the USPTO/Espacenet dead-man search is done; the bridge waits on a `synapsed-client` publish newer than rc.123): ROADMAP A10, `POST /alert` per-key rate limit (module `ratelimit`, defaults 6/h, 20/day, `ALARMANT_RATE_PER_HOUR`/`_DAY`); red-first test seen failing, sabotage fails 5 tests, 206 tests, fragment `changelog.d/alert-rate-limit.md` (BREAKING for a sender over 6/h). PR day: 2026-11-22 UTC (11-21 is the note's); [READY] to pm-assist cc pm-ants.
- PR #73 merged as 0.3.1 (note `docs/SYNAPSE-ADAPTER.md` in main; branch and worktree removed). No adapter/watchdog code until CireSnave answers (board 478); owed: USPTO full text and Espacenet heartbeat search (a person or a lane with access), a person confirming the readings.
- A8 (#70) merged: first `android-v*` tag push waits for the PM's word and CireSnave's five secrets (board 462).

PR #74 OPEN (0.4.0 stamped by pm-assist; gate armed): carries A10, GET /alert/:id, tests/constraints.rs, `bridge` module (pure logic, not wired), PATENT-SYNAPSE-READING.md + addendum S5-S10 / constraint S7, Apache text. Pull before any push, no force; no fragment file (stamp consumed it; add lines by hand under the CHANGELOG heading while #74 is open). Portfolio sweep red also on main@0bd783d. Keep committing on `alert-rate-limit`; next window's PR is cut from it.

NEXT, in order:
1. 11-21 UTC: open the note PR; run and record the patent reading (Part: inbound channel and heartbeat watchdog).
2. 11-22 UTC: open the `alert-rate-limit` PR.
3. Add ROADMAP rows A11 (PinTrustManager handshake test) and A12 (Windows key ACL) on the note branch.
4. Phone-session readiness needs a real device (board 354 Firebase, CireSnave).
5. Owed: a person's confirmation of the patent readings (outside as read); the sweep is incomplete.

Tools: `C:\Projects\tools\patent-claims.py <numbers>`; emulator AVD `alarm36` in `C:\Users\cires\lane-tools\alarmant`. Windows: use `python tools/check-fragment.py origin/main`; run `cargo fmt --check` before every push (CI fails on it). Many files here are CRLF: edit with a line-ending-aware script (`grep -c $'\r'` lies on this box).
WAIT: #74 merge | check: gh pr view 74 --json state,mergedAt | wake: 2026-11-22T13:10:00+00:00
WAIT: synapsed-client newer than 6.0.0-rc.123 on crates.io (then wire `alarmant-bridge` around `bridge`) | check: curl -s https://crates.io/api/v1/crates/synapsed-client | wake: 2026-11-22T13:20:00+00:00
OPEN ASK to pm-ants (2026-11-22): may watchdog code start inside S4, S6, S7 on the reachable search? Until answered: NO watchdog code.
