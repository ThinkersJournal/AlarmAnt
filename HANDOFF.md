# HANDOFF: AlarmAnt

STATE (2026-11-24, main 0.4.1 after #75): FULL SPEED. Branch lane/alarmant-bridge (worktree C:\Projects\AlarmAnt-wt-bridge) has `alarmant-bridge` built (pm-ants [FIX]/[TASK] 2026-11-24): bin, `bridgeclient`, parse_envelope/utc/replies in `bridge`, synapsed-client rc.266, doc 3.1 restated, CN122885272A/CN122970019A paragraphs, fragment; 226 tests + clippy green; NOT run against a live daemon or server. Main has the whole server side of ruling 389 (chain, persisted stores, admin endpoint, `alarmant`, `alarmant-pc`, Android app, rate limit, `GET /alert/:id`). Nothing run live: needs a Firebase service account (board 354). Portfolio sweep is red on main and PRs (cause not isolated).

RULES (PM): phone never reports "received"/"viewed", only Acknowledge; no Decline or negative status (PATENT-SWEEP constraints 13-22); escalation on a fixed timer only; any change needs a fresh patent reading and the PM's OK. Say "outside as read" (and "partial search"), never "clear"; not legal advice. Secrets never in repo, argv or logs.
Workflow: Sonnet only; the PM stamps the version at merge and consumes the fragment; never edit Cargo.toml version or Cargo.lock (a new dependency's lock entry excepted). Pull before push (merge, never force: the PM stamps onto the branch). Sign PR bodies `— lane: alarmant`. Primary stays on `main`; branches in `C:\Projects\AlarmAnt-wt-<topic>`. One PR per UTC day. Messages: compact format. Clean worktree+branch after each merge. crates.io needs a User-Agent on curl (`-A`).


NEXT:
1. Push lane/alarmant-bridge; the day PR opens 2026-11-25 00:00Z or later (150k diff ceiling: Cargo.lock grows ~2000 lines for synapsed-client; check `git diff --stat origin/main`). Send [READY] to pm-assist cc pm-ants. Open items: a live daemon test (SYNAPSE_HOME, role alarmant); a pinned-subject/ack-on-behalf swap if Synapse builds it; README/run notes for the bridge env vars.
2. Board 490 (CireSnave): A runs queries Q1-Q4, B hold, C proceed; the PM recommends B. NO WATCHDOG CODE (ruling 478).
3. A13: quarterly note to the PM about 19/662,313 (next 2027-02-21). A8: first `android-v*` tag waits on the PM and five secrets (board 462). Phone readiness needs a real device (board 354).
4. Owed: a person's confirmation of the patent readings (partial); US 13374271 scanned claims need a person's eye; USPTO ODP claim reads for row 20 (antspawn) not done, only Google (sent to pm-ants 2026-11-23).

Tools: `C:\Projects\tools\patent-claims.py US<number><kind>`; Google Patents xhr query works keyless; emulator AVD `alarm36` in `C:\Users\cires\lane-tools\alarmant`. `python tools/check-fragment.py origin/main` reads COMMITTED state only; run `cargo fmt --check` and clippy before every push. Many files are CRLF.
