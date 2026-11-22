# HANDOFF: AlarmAnt

STATE (2026-11-22, main 0.4.0 after #74): FULL SPEED. Main has the whole server side of ruling 389: escalation chain first app -> second app -> PC alarm (`escalate`, `wire`, `channel`, `second`), persisted stores (`persist`), loopback admin endpoint + `enroll`/`revoke` client (`adminapi`, `admin`), the `alarmant` binary (`serve`, `enroll|revoke --contact first|second`), the PC alarm program `alarmant-pc` (Windows; `pcclient`, `POST /pc/ack/:id`), the Android app, the `/alert` rate limit, `GET /alert/:id`, the `bridge` module (pure logic, not wired) and the patent readings. Nothing has been run live: needs a Firebase service account (board 354, CireSnave).

RULES (PM): phone never reports "received"/"viewed", only Acknowledge; no Decline or negative status (PATENT-SWEEP constraints 13-22: server never records a phone or PC fetch; PC sends only Acknowledge, no status); escalation on a fixed timer only; any change needs a fresh patent reading and the PM's OK. Say "outside as read" (and "partial search" where it is), never "clear" (sweep incomplete; not legal advice). Secrets never in repo, argv or logs.
Workflow: Sonnet only; the PM stamps the version at merge and consumes the fragment (no fragment file while a PR is open: add CHANGELOG lines by hand under the stamped heading only when told); never edit Cargo.toml version or Cargo.lock (a new dependency's lock entry is the one exception). Pull before any push, never force. Sign PR bodies `— lane: alarmant`. Primary stays on `main`; branches in `C:\Projects\AlarmAnt-wt-<topic>`; after a squash-merge use a merge commit, never force-push. One PR per UTC day, then keep building on the branch. Messages: compact format, `key=` pairs only. Clean worktree+branch after each merge.

IN FLIGHT:
- #74 MERGED (0.4.0, 02:51Z): A10 `/alert` rate limit (BREAKING), `GET /alert/:id`, `tests/constraints.rs` (ruling 407-A guard), `bridge` module (admission caps, open-alert file, audit line; NOT wired), PATENT-SYNAPSE-READING.md (+ addendum S5-S10, constraint S7, queries Q1-Q4 for a person), Apache text. Branch and worktree removed. The portfolio sweep was red on main too (cause not isolated).
- Branch `adapter-bridge` (worktree `AlarmAnt-wt-adapter`, pushed, no code yet): for `alarmant-bridge` (second binary) once a published `synapsed-client` newer than 6.0.0-rc.123 exists (crates.io has only rc.123, 2026-10-21). Plan from `docs/SYNAPSE-ADAPTER.md` 3.2 and 3.6: claim role `alarmant`, read mail, `Gate::admit`, `POST /alert`, poll `GET /alert/:id` every 20 s up to 24 h, send `[NOTICE] page-acked` / `page-refused` replies (reply ids derived from the alert id); on 409 `superseded` stop, never re-claim.
- NO WATCHDOG CODE (ruling 478, PM): board 490 is CireSnave's choice (A he runs queries Q1-Q4, B hold, C proceed; the PM recommends B).
- Rulings in force: 407-A (proceed inside constraints 13-16; slips to refuse: slice (h) reporting "unregistered" or "permissions missing" to the escalator, any "FCM push failed, escalate now" shortcut). ROADMAP A13: quarterly note to the PM asking CireSnave about 19/662,313 (next 2027-02-21).
- A8 (#70) merged: the first `android-v*` tag push waits for the PM's word and CireSnave's five secrets (board 462). ROADMAP A11 (PinTrustManager handshake test) and A12 (Windows key ACL) are open.

WAIT: synapsed-client newer than 6.0.0-rc.123 on crates.io (then wire `alarmant-bridge` around `bridge`; pm-ants mails when it publishes) | check: curl -s https://crates.io/api/v1/crates/synapsed-client | wake: 2026-11-23T13:20:00+00:00

NEXT, in order:
1. When synapsed-client publishes: build `alarmant-bridge` on `adapter-bridge`.
2. A11 and A12 if no client yet.
3. Phone-session readiness needs a real device (board 354 Firebase, CireSnave).
4. Owed: a person's confirmation of the patent readings (outside as read, partial); the sweep is incomplete.

Tools: `C:\Projects\tools\patent-claims.py US<number><kind>` (needs the kind code, e.g. US9351640B2); Google Patents keyword search works through `patents.google.com/xhr/query?url=q%3D...`; emulator AVD `alarm36` in `C:\Users\cires\lane-tools\alarmant`. Windows: `python tools/check-fragment.py origin/main` reads COMMITTED state only; run `cargo fmt --check` and clippy before every push. Many files here are CRLF: edit with a line-ending-aware script or the Edit tool.
