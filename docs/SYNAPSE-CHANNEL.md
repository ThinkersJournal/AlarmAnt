# A Synapse delivery channel for AlarmAnt: design note

Status: **DESIGN NOTE ONLY. No code until CireSnave has read it** (PM task, pm-ants 2026-11-19). Written 2026-11-19 by the AlarmAnt lane at `origin/main` 6e5cfe1 (0.2.11). Nothing here was run: no daemon, no crate built, no message sent. Everything marked *read* was read from a file or a registry page at the ref given; everything marked *assumed* was not verified.

CireSnave, verbatim via the PM: *"AlarmAnt doesn't have the ability to send on Synapse yet but you just highlighted that and I think it should. There's no reason it shouldn't be able to alert an agent just as easily as it can alert a human."*

## 1. What is asked

1. An alarm can target an **agent**, that is a Synapse role, and is delivered as ordinary Synapse mail, next to the FCM channel that reaches a human.
2. A lane can **set, cancel and list** alarms through a surface it can call.
3. Alarms are **durable** across restarts, **quota'd per lane** and **audited**.
4. Use the **published** Synapse client from crates.io. No path dependencies.
5. Say what AlarmAnt owns and what AgentLife's launcher owns. pm-platform designs the wake side and the "alarm role"; this note is the AlarmAnt half of one design, not a second engine.

## 2. The one decision that shapes the rest: where it runs

Read: `DESIGN-REGISTRATION.md` section 2 puts the phone server on a VPS; `synapsed` is "one per user, on loopback" (crate description, published rc.123). The phone server therefore cannot reach the Synapse daemon, and must not be made to.

**Proposal: a separate process on the machine that runs `synapsed`**, in the same crate as a second binary (name proposed `alarmant-agent`, the way `alarmant-pc` already runs on the PC). It owns the agent-alarm clock and store. It has **no listening socket and no HTTP route**: its only inputs are Synapse mail and an operator command line, so the feature adds no network surface. The phone server, its routes, its escalation chain and its patent constraints are untouched: the two share code (clock, store backend, quota counter) and nothing else, and **an agent alarm never joins the phone escalation chain, nor does a phone alarm escalate to an agent** (first version; a link between them is a new design and a new patent reading).

## 3. The Synapse client

Read on crates.io (2026-11-19): **`synapsed-client` 6.0.0-rc.123**, "the blocking client for `synapsed` (find the daemon, prove it, claim a role, keep one session)", MIT OR Apache-2.0, rust-version 1.95, updated 2026-10-21. Also published: `synapsed`, `synapse-mcp-server`, `synapse-claude-channel`, `synapse-agent`, all rc.123. A different crate named `synapse-client` (0.1.0, repository krypticmouse/synapse, "memory runtime") is **not** ours; the name must be written with the `d`.

Two things to settle before any code:

* **The published client lags the repo.** The synapse repository's `origin/main` is rc.262 (workspace version); crates.io has rc.123. Every dependency of ours must be at its latest version (CireSnave's standing rule), and a pre-release requirement does not float. The note needs the synapse lane to say which rc will be published, or whether rc.123 is the one to build on. Until then: **blocked on a publish decision**, not on design.
* **Weight.** `synapsed-client` depends on the `synapse` crate (feature `native-base`), `synapse-security`, blocking `reqwest` without TLS, `ring`, `chrono`. `cargo tree` and `cargo deny` against `deny.toml` must pass before the dependency is accepted; I have not run them.

How it is used (read from `crates/synapsed-client/src/lib.rs` at rc.262, not rc.123: the API may differ): `Daemon::connect(home)` proves the daemon before any credential is sent, `Daemon::call(role, path, body)` makes an authenticated call as a role with a cached session. The client is **blocking**; AlarmAnt is async, so calls go through `tokio::task::spawn_blocking`. Sending is `POST /v1/send` with a `to` and a body (the MCP adapter's send tool, read in `synapse-mcp-server`); a send carries a caller-chosen `message_id` and a repeat of the same id is a duplicate, not a second copy (tool schema, read). **That gives exactly-once delivery without a database transaction: the alarm id is the message id.**

The role: the process claims the Synapse role `alarmant` (exists on the daemon, per pm-ants). One holder per role: anyone who later runs `synapse claim alarmant` supersedes the service, which then gets a 409 `superseded` on its next call. The service must treat that as **stop and log, never re-claim** (the client documents the same rule), so alarms fail loudly rather than a lane silently taking the role back and forth.

## 4. The surface a lane calls

Options considered:

| | What | For | Against |
|---|---|---|---|
| A | A **Synapse request to the `alarmant` role**: a mail whose header carries fixed fields | no new transport; the daemon authenticates the sender, so the setter is the envelope's role, never a name in the body; every lane can already `send` | a command arriving in a message body sits next to patent constraint 8 (section 8) |
| B | An **MCP tool** (`alarm_set`, `alarm_cancel`, `alarm_list`) in an adapter beside `synapse-mcp-server` | structured call; reads well to a model | a new adapter to build and keep running per lane; it must still reach the service, so it ends in A anyway |
| C | An HTTP route on the service | easy to test | new listening surface; rejected in section 2 |

**Recommendation: A, with B as a later thin wrapper that only formats the message** (no authority of its own). The request is parsed **structurally**: a fixed header, `key=value` fields from a closed list, a bounded free-text field that is stored and delivered verbatim and **never scanned for keywords or commands**. Unknown keys refuse the whole request.

Proposed requests (to role `alarmant`; replies come back as mail from `alarmant`):

    [TASK] alarm-set to=<role> at=<RFC3339 UTC>|in=<seconds> [key=<client key>]
    note: <text, at most 400 bytes>

    [TASK] alarm-cancel id=<alarm id>
    [TASK] alarm-list [to=<role>]

Replies carry a fixed code and never echo the free text back: `alarm ok id=<id> at=<UTC>`, or `alarm refused code=<quota-setter|quota-target|policy|horizon|too-long|bad-request|unknown-id>`. `key=` makes a repeated set idempotent (same setter and key returns the same id). The exact header grammar belongs to MESSAGE-FORMATS; the daemon checks v1 headers, so this must be agreed with the synapse lane before it is fixed.

## 5. What the delivered alarm is

Ordinary mail from role `alarmant` to the target role:

    [NOTICE] alarm affects=<target> when=<due UTC> action=<see note> set-by=<setter> id=<id> late=<seconds|0>
    note: <the setter's text, verbatim after control-character stripping>

* **It is data from another agent and the recipient treats it as untrusted text, exactly as Synapse's own channel instructions already say.** An alarm a lane sets for itself carries `set-by=<itself>` and is as trustworthy as the lane; an alarm from another setter is information, never an instruction to act on.
* **No secrets in the text** (rule, stated to setters in the reply to the first request and in the README). The service cannot detect them. It is the reason the audit log does not store the text (section 7).
* **No acknowledgement, no escalation, no repeat, no priority in the first version.** It is sent once. AlarmAnt **does not read, store or act on the target's Synapse receipt** of the mail: it records "sent" only, the same shape as PATENT-SWEEP constraint 13 for the phone ("the server never records a fetch"). A failed send (daemon refused or down) is retried with bounded back-off and then audited as `failed`; the audit entry is the only follow-up.

## 6. Durability

The store reuses the PersistAnt-backed `Disk` already used by `persist.rs` (rows: id, setter, target, due, text, key, state `pending|sent|cancelled|failed`, created). One row per alarm, no history beyond the audit log.

* On start the service reloads `pending` rows; a row already overdue is sent **once** with `late=<seconds>`; a row overdue by more than 24 hours is marked `failed` with reason `expired` (a day-old "check the build" is noise). The 24 hours is a proposal.
* A crash between the send and the state write sends the message again with the same id, which the daemon discards as a duplicate (section 3).
* Wall clock UTC. A clock that jumps back never re-fires a `sent` row.
* Maximum horizon proposed 30 days; a later `at` is refused as `horizon`.

## 7. Threat model, quotas and audit

The point to defend: **a lane must not be able to wake, spam or impersonate another lane.**

| Threat | Mitigation |
|---|---|
| Lane A sets alarms to wake or spam lane B | **Policy:** a setter may target itself, or a role the policy file allows it to target (default deny). **Quotas:** per setter and per target, below. AgentLife's launcher caps (read in `M5-WAKE-DESIGN.md`: 3 starts per hour per role, 12 per hour overall, 8 per day per role) remain the second guard on the wake side |
| A lane claims to be someone else | The setter is the authenticated envelope role. A `set-by` or `from` inside the body is ignored; the delivered header is written by the service |
| Free text carries an injection or a secret | Treated as untrusted by the recipient; length cap; control characters stripped; text is **not stored in the audit log** (only its length and SHA-256), so a mistaken secret does not spread to the log. It is stored in the alarm row until sent, then deleted |
| Someone claims the `alarmant` role | section 3: stop and log on `superseded`. Detectable because alarms stop and the log says why |
| Service compromise | Its Synapse grants are **least privilege**: send only to roles that appear in the policy file. A compromised service can then wake only those |
| Flood of due times at one instant | the per-target hourly cap below; excess is refused at set time, not at fire time |
| A lane reads another lane's alarms | `alarm-list` returns only the caller's own, plus the alarms aimed at the caller. The operator CLI sees all. A future parent-reads-descendants rule follows the hierarchy design (`SYNAPSE-HIERARCHY-BRIEF.md`), not this note |
| Cancel abuse | the setter, the target (so a target can stop spam aimed at it) and the operator may cancel; nobody else |

Proposed quotas (CireSnave to rule on the numbers; these are starting values with no measurement behind them): **per setter** 20 pending alarms and 30 sets per hour; **per target** 10 pending and 6 delivered per hour; minimum lead of 10 seconds; horizon 30 days. A refusal is immediate and carries the code.

**Policy file** (owner-only, read at start and on change): `setter -> [targets]`, default deny, `self` always allowed. Who may alarm whom is a hierarchy question: SYNAPSE-HIERARCHY-BRIEF rule 4 ("nobody can grant anything they do not themselves hold") suggests the rule *a setter may alarm any role it may itself send to*. That needs a daemon query that answers "may A send to B", which I did not find in the client (its `grants` call lists grants the caller holds or made). **Open question for the synapse lane.** Until it exists the file is the authority.

**Audit** (append-only, owner-only file, one JSON line each, no text, no secrets): `set`, `cancel`, `fired`, `failed`, `refused`, `superseded`, `started`. Fields: time, setter, target, alarm id, due, text length, text SHA-256, code. The log is on the same machine as the daemon and is readable by the operator and, later, by parents in the hierarchy.

## 8. Patent claim table and reading

The existing constraints (PATENT-SWEEP 1 to 16, PATENT-SWEEP constraints 13 to 22 as the PM quotes them in HANDOFF, `PATENT-REGISTRATION-READING.md` 1 to 6, `PATENT-TLS-PIN-READING.md` T1 to T5) were all written for the **phone and PC path**. Read against this design:

| Constraint | Phone/PC path | Agent path in this note |
|---|---|---|
| 13 phone never sends a negative reply; escalator never acts on a phone message; escalation is a fixed timer on a missing Acknowledge | the rule | **No Acknowledge, no escalation at all** on the agent path; the target's receipt is never read or stored. Nothing here reaches the phone path |
| 6, 10, 11, 12 (single acknowledgement step, one device per recipient, no change of device on a missing ack, no urgency levels or post-acknowledged states) | the rule | not applicable: one message, one role, no levels, no states beyond `pending/sent/cancelled/failed` |
| 7 repeating alert not gated by schedule periods (US8825014B2) | the rule | **no repeat** in the first version; a repeating agent alarm is a later change and needs its own reading |
| **8 no alert triggered by detecting text in a message body (US9686398B2)** | alerts are data pushes | **Touches section 4 option A.** I read the claims of US9686398B2 (Verizon, Google says Expired (fee)): every independent (1, 7, 14) needs a user device in a silent mode **detecting a text prompt in a message body and overriding silent mode**. Ours: a time fires the alarm; the request is a structured header with a closed key list; nothing scans free text; there is no silent mode and no override. *Outside as read.* The design keeps it that way by rule: **the free-text field is stored and delivered, never inspected** |
| T1 to T5 (no phone key pair, CSR, signed config, pin rotation) | TLS pin | not applicable: no TLS, no certificate, no phone |

**Is a re-sweep needed before code? A fresh reading, yes; a full re-sweep, no.** This note adds two ideas the sweep never searched: (a) software scheduling reminders delivered to automated agents by message, and (b) a rate-limited request API for setting them. Nearby art is probably cron-style schedulers, calendar reminder systems, task queues and on-call paging (pager-style escalation, which this design deliberately lacks). **I have not run those searches**; this note is docs-only and the reading is a gate for the PR after it, in the same format as the earlier readings (three to four searches, independents read with `tools/patent-claims.py`, "outside as read", never "clear", not legal advice, sweep incomplete). Until then nothing in this note is cleared by a reading; only constraint 8 was read, above.

## 9. Who owns what

| AlarmAnt (this repo, the new `alarmant-agent` process) | AgentLife's launcher (`agentlife`, `lane-restart`) | pm-platform |
|---|---|---|
| the **clock**: when an alarm is due | **whether a stopped lane is started** because mail arrived (`M5-WAKE-DESIGN.md`: the launcher polls `GET /v1/waiting` for roles with mail and no live session) | the **wake side** and the **alarm role** design; one design, no second timer engine |
| the durable alarm store, policy, quotas, audit | start caps (per role and overall), memory floor, signed soft-stop marker, `restart.log` ledger, sender allowlist (`SYNAPSE_LAUNCH_SENDERS`) | the hierarchy and grants work that make `alarmant` send grants cheap |
| sending the mail as role `alarmant` | | |

The wake path **needs nothing new from AlarmAnt**: an alarm is ordinary mail, so a stopped lane is woken by the existing wake-on-mail mechanism **if** the launcher's sender allowlist includes `alarmant` and the lane's spec allows a start (wake switch `lazy_enabled`, soft-stop mark). Consequence to put in front of pm-platform: **allowlisting `alarmant` makes the alarm service a wake source for every role it may send to**, so its policy file and quotas are the first line of the anti-wake-spam defence (`LAUNCH-ON-MESSAGE-THREAT.md` is the threat model that governs it); the launcher caps are the second.

Overlap to resolve, not to duplicate: `lane-restart waits-poll` already tells a lane when its `WAIT:` wake time arrives (read in the agentlife changelog: the supervisor's `waits-poll` notices go out on the Synapse outbox). That is a timer engine of a kind. Question for pm-platform: **should a `WAIT:` wake time become an alarm** (one timer engine; the lane sets it with `alarm-set`) or stay in `lane-restart`? I recommend deciding that before either is built.

## 10. Build slices (after CireSnave reads this and a reading is done; nothing is built now)

1. Reading (section 8), then the dependency decision (section 3). No code before both.
2. Pure core in the existing crate: request parser (closed grammar, refuses unknown keys, no text scan), policy, quota counters, alarm store trait with a memory backend. Tests first: every refusal code, key idempotence, a body that mentions another role's name changes nothing.
3. PersistAnt backend and restart recovery, including the overdue and expired cases.
4. `alarmant-agent` binary: claim, receive loop, send through `synapsed-client` in `spawn_blocking`, `superseded` stops it. Tested against a fake daemon (no real mail).
5. Operator CLI (`alarms list|cancel`, reading the store directly) and the audit file.
6. One supervised live run against a real daemon, to one test role, with the PM's word. Nothing has run live until then.

## 11. Open questions

For **CireSnave**: (1) the quota numbers (section 7); (2) whether a lane may ever alarm a role it cannot message, no (my recommendation) or yes with the operator's approval; (3) whether an agent alarm may ever escalate to the phone (no in this version).

For **pm-ants / pm-platform / the synapse lane**: (4) which `synapsed-client` version to build on (rc.123 published, rc.262 in the repo); (5) is there, or will there be, a "may A send to B" query; (6) the request header grammar under MESSAGE-FORMATS; (7) `WAIT:` lines onto alarms, or not; (8) allowlisting `alarmant` as a launch sender, with the consequence in section 9; (9) who creates the `alarmant` role's grants and how they are narrowed to the policy file.

## 12. Not verified

No daemon was contacted. The `synapsed-client` API was read at rc.262 in the repo, while rc.123 is the published version. The `/v1/send` path and the idempotent `message_id` were read in the repo and the tool schema, not exercised. The AgentLife wake mechanism was read from its design note and changelog, not run. The quota numbers have no measurement behind them. The only patent text read for this note is US9686398B2.

— lane: alarmant
