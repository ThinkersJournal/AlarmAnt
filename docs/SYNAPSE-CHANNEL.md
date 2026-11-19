# Paging a human from Synapse: design note

Status: **DESIGN NOTE ONLY. No code until CireSnave has read it.** Written 2026-11-19 by the AlarmAnt lane at `origin/main` 6e5cfe1 (0.2.11). It **replaces** an earlier version of this note that also designed agent alarms, set/cancel/list and per-target quotas; pm-ants reshaped the task the same day and that material is gone. Nothing here was run: no daemon, no crate built, no message sent. *Read* means read from a file or registry page at the ref given; *assumed* means not verified.

Scope, from the PM (CireSnave 2026-11-19): **agent to human only.** A lane sends Synapse mail to the role `alarmant`; AlarmAnt raises its existing escalating human alert (FCM push, second app, PC alarm) for it; the acknowledgement comes back as Synapse mail. **AlarmAnt stores no alarms and has no agent-alarm path.** CireSnave, verbatim via the PM: *"I agree that the alarm clock itself belongs in Synapse and if it's already in Synapse, it seems counter productive for it to call AlertAnt to send an alert over Synapse."* The Synapse clock appears here only as one more **sender** (section 6).

## 1. Where it runs, and why there is a new small process

Read: `DESIGN-REGISTRATION.md` section 2 puts the phone server on a VPS; `synapsed` is "one per user, on loopback" (crate description, published rc.123). The phone server cannot reach the daemon and must not be made to.

**Proposal: a bridge process on the machine that runs `synapsed`** (second binary in this crate, name proposed `alarmant-bridge`, in the way `alarmant-pc` already runs on the PC). It claims the Synapse role `alarmant`, reads mail addressed to it, and for an accepted page calls the **existing** `POST /alert` on the phone server over https with the existing API key. It has **no listening socket**: inputs are Synapse mail only. It keeps **no alarm and no schedule**; its only state is the open pages it is waiting to hear acknowledged (section 5), and that is a cache of what the server already holds.

One small addition on the server is needed for the ack path (section 5): a read of one alert's acknowledged-or-not state.

## 2. Who may page a human

A page is the most intrusive thing in the portfolio (it overrides silent mode and sounds a PC), so the gate is **who**, then **how often**; it is not a severity scale.

* **Authority is a Synapse send grant to `alarmant`.** The bridge accepts mail only from a role the daemon authenticated as the sender; a name written in the body counts for nothing. **Default: nobody holds the grant.** CireSnave names who does. Proposal: the PM and the cluster PMs, never an ordinary lane, so a lane that needs a human asks its cluster PM, who decides. (Mail from a role without the grant never reaches the role; the daemon refuses it.)
* **No severity levels.** AlarmAnt has one priority by design (PATENT-SWEEP constraint 12: no urgency levels, threshold types or post-acknowledged states). A `severity=` field would add exactly that, and a closed `reason=` list would behave as one. So the page carries no level and no category. What keeps a status update from paging at 3 am is the allowlist above, the rate limits below, and a written rule for senders (section 8), not a field the sender fills in.
* **No quiet hours in this version.** A time-of-day gate is the other thing that would let "not at 3 am" be automatic; it also resembles PATENT-SWEEP constraint 7 (a repeating alert gated by schedule periods, US8825014B2, which was about a battery setting combined with schedule times). I have not read a claim against a plain suppression window, so I am not proposing one. **Open question for CireSnave**, section 11.

Proposed limits (starting values, no measurement behind them; CireSnave rules on the numbers): **per sender** 1 open page and 3 pages a day; **overall** 3 open pages and 10 a day. A page over a limit is refused at once with a code, never queued, so a burst cannot become a delayed storm. `open` means raised and not yet acknowledged.

## 3. Message format (MESSAGE-FORMATS style)

New header type, one line then at most three `note:` lines, as `MESSAGE-FORMATS.md` requires (values without spaces; the type is versioned in its tag):

    [PAGE v1] <subject> to=human blocks=<what waits, one token or ->
    note: <one idea per line, at most 3>

* `<subject>` becomes the alert **title**, at most 100 characters (the existing `MAX_TITLE_CHARS`). The notes joined with a space become the **body**, at most 1000 characters (`MAX_BODY_CHARS`, FCM's 4 KB cap).
* The **bridge** prefixes the title with the authenticated sender (`pm-ants: ...`) so the human sees who is paging and the sender cannot dress it as someone else.
* Control characters are stripped. The text is **never inspected**: nothing is searched for keywords, nothing in it changes what the bridge does. Fields come from a closed list and an unknown key refuses the page.
* **No secrets.** The text crosses Google's push service and lands on a lock screen (read: `fcm.rs` posts to `fcm.googleapis.com`). Say the board number or PR, not the content.

Replies, all from role `alarmant` to the sender, fixed text:

    [PAGED] <subject> id=<alert id> at=<UTC>           the alert was raised
    [REFUSED] page code=<no-grant|sender-limit|open-limit|day-limit|too-long|bad-request|server-down|no-device>
    [ACKED] <subject> id=<alert id> at=<UTC>           the human acknowledged

`no-device` and `server-down` map the server's existing answers (409 `NoDevice`, push-service failures). A refusal never echoes the text.

## 4. The published Synapse client

Read on crates.io (2026-11-19): **`synapsed-client` 6.0.0-rc.123**, "the blocking client for `synapsed`", MIT OR Apache-2.0, rust-version 1.95, updated 2026-10-21. Written with the `d`: a different crate called `synapse-client` (0.1.0, repository krypticmouse/synapse, "memory runtime") is **not** ours. No path dependencies.

Open before code:

* **The published client lags the repo.** The synapse repository's `origin/main` is rc.262; crates.io has rc.123. A pre-release requirement does not float, and CireSnave's rule is latest versions. The synapse lane has to say which rc gets published or whether rc.123 is the one to use. **Blocked on a publish decision, not on design.**
* **Weight and licences.** It depends on the `synapse` crate (`native-base`), `synapse-security`, blocking `reqwest` without TLS, `ring`, `chrono`. `cargo tree` and `cargo deny` must pass; I have not run them.

Usage, read in `crates/synapsed-client/src/lib.rs` at rc.262 (the API may differ at rc.123): `Daemon::connect(home)` proves the daemon before any credential goes out; `Daemon::call(role, path, body)` makes an authenticated call as a role. It is blocking, so the async bridge calls it through `spawn_blocking`. Reading mail and sending are daemon calls (`/v1/send` read; the fetch and ack paths assumed from the MCP adapter's tools). A send carries a caller-chosen `message_id`; a repeat is a duplicate, not a second copy (tool schema, read), so **a reply's id is derived from the alert id and a re-send after a crash is harmless.**

Holding the role: one holder per role. If someone runs `synapse claim alarmant`, the bridge gets 409 `superseded`; it **stops and logs and never re-claims** (the client documents the same rule). Pages then fail loudly with silence, not by two processes fighting for the role. **Risk to name:** a stopped bridge means agents' pages are not raised; senders get no `[PAGED]`, which is the signal.

## 5. The acknowledgement path

1. The bridge raises the alert with `POST /alert` and receives `{"alert_id": ...}` (read: `raise.rs`), then sends `[PAGED]`.
2. The phone (or the PC alarm) acknowledges to the server as today (`POST /ack/:id`, `POST /pc/ack/:id`). **Nothing changes there.**
3. The bridge needs to learn that. The server cannot call the PC, so the bridge asks: a new read-only route **`GET /alert/:id`** with the same API key, answering `{"state":"open"}` or `{"state":"acked"}` and **nothing else**: not the escalation step, not whether a push was sent, not any phone or PC fetch. The bridge polls each open page (proposal: every 20 seconds, for at most 24 hours) and sends `[ACKED]` when it reads `acked`.
4. The sender hears nothing else. **No "delivered", "seen", "escalated" or "chain finished" mail**, in line with PATENT-SWEEP constraint 13 (the server never records a phone or PC fetch; the only input is Acknowledge) and constraint 12 (no post-acknowledged states). A sender that wants to know whether a human reacted reads `[ACKED]` or its absence. **Open question 11.3** whether silence after a long time needs an explicit `[UNACKED]`; my recommendation is no.

The new route is a **read of state the server already holds** (the alert store's `Acked` flag, used today by the PC board). It is VPS-facing and needs the API-key check, a rate limit and a fixed-shape answer, and tests first (unknown id answers 404 with no hint; wrong key answers the same 401 as `POST /alert`).

## 6. The Synapse clock as a sender

When Synapse grows a scheduled send (pm-platform and the synapse lane own it), a timed page is **just another message to `alarmant`** and meets every rule in section 2. The one requirement AlarmAnt must put on that design: **a scheduled send is attributed to the role that scheduled it, not to "the clock"**, and is accepted only if that role holds the grant when it fires, not when it was set. If it were attributed to the clock, any role could page a human by scheduling a message and borrowing the clock's authority. I do not know how the synapse lane intends to attribute it; **ask**, section 11.

## 7. What is audited

An append-only, owner-only file beside the bridge, one JSON line per event, **names and codes only**: time, event (`received`, `raised`, `acked`, `refused`, `superseded`, `started`), authenticated sender, alert id, refusal code, and for a received page the **length and SHA-256 of the text, not the text**. Not logged, ever: the API key, any Authorization header, the Synapse session, the text of a page. The phone's own alert log already keeps what the human saw.

## 8. Rule for senders (to go in the README and the grant message)

Page a human only when a decision or action by CireSnave is needed and cannot wait for the next time he reads the board. A status, a finished PR, a question that can sit on the board, a failed CI run that a lane can fix itself: not a page. Say what is needed in the subject. The allowlist, not goodwill, is the enforcement; this sentence is what a lane is told when it is given the grant.

## 9. Threat model

| Threat | Mitigation |
|---|---|
| A lane pages CireSnave for a status | send grant is default-deny and held by few; per-sender and overall limits; refused pages are audited; the sender is named on the alert |
| A role pages without the grant | the daemon refuses mail to `alarmant` from a role with no send grant; the bridge also checks the authenticated sender against its own allowlist (belt and braces, since the grant is the single point of authority) |
| A role impersonates another | the sender is the daemon's authenticated envelope role; nothing in the body is believed; the bridge writes the sender into the title |
| Free text carries an injection or a secret | the text is data, never inspected or acted on; length caps; control characters stripped; text not in the audit log; senders told not to include secrets |
| Page storm | one open page per sender, a small overall open limit, daily caps, refusal not queueing |
| The bridge's API key leaks | held in a file read at start (never argv, env dump or log), as the admin key already is; the key can only raise alerts, never register a device (`DESIGN-REGISTRATION.md` section 3) |
| Someone takes the `alarmant` role | stop and log on `superseded`; silence is detectable |
| A compromised bridge | its Synapse grants are least privilege (send only to the allowed pagers for `[PAGED]`, `[ACKED]` and `[REFUSED]`); its server credential can only raise alerts and read one alert's open/acked state |
| A clock or scheduler launders authority | section 6 |

## 10. Patent claim table and reading

All current constraints (PATENT-SWEEP 1 to 16, `PATENT-REGISTRATION-READING.md` 1 to 6, `PATENT-TLS-PIN-READING.md` T1 to T5) were written for the phone and PC path, which **this design does not change**: the escalation chain, the phone's one-step Acknowledge, the PC alarm and the registration flow are untouched. Read against the new inbound channel:

| Constraint | Effect of this design |
|---|---|
| 13 (phone sends no negative reply; escalation is a fixed timer on a missing Acknowledge; server records no fetch) | kept: the only thing the sender learns is that an Acknowledge happened; the new `GET /alert/:id` reveals nothing else |
| 12 (no urgency levels, threshold types or post-acknowledged states) | kept **by leaving out** severity, categories and a status ladder (section 2, section 5) |
| 7 (repeating alert gated by schedule periods) | no repeat added; quiet hours deliberately not proposed (section 2) |
| 6, 10, 11 | unchanged: one acknowledgement step, one device per recipient, no change of device on a missing ack |
| **8 (no alert triggered by detecting text in a message body; US9686398B2)** | **this is the closest this feature comes, so it is the one I read.** US9686398B2 (Verizon; Google says Expired (fee)), independents 1, 7, 14: **a user device, in a silent mode, receives a message, detects a text prompt in its body and, on that detection, provides an alert that overrides the silent mode.** Ours: the message is not addressed to the user's device; it goes to a service role and is accepted by **who sent it and the grant they hold**, with a fixed protocol header; nothing scans free text for a prompt; the human's phone receives a data push from the existing path and detects nothing in any message. *Outside as read.* The text-ignored rule in section 3 is what keeps it so. A person's confirmation is owed, like the other readings |
| T1 to T5 | not touched: no certificate, key pair or pin on this path |

**Is a re-sweep needed before code? A fresh reading, yes; a full re-sweep, no.** This note adds an inbound machine-to-service channel that raises a human alert, and a rate- and sender-limited request. Nearby art is likely on-call paging and incident tools (pager escalation, which AlarmAnt's chain already differs from), alert-forwarding gateways, chat-ops bots and notification rate limiting. **I have not run those searches**: this note is docs-only, the reading is a gate for the PR after it, in the format of the earlier readings (a few searches, independents read with `tools/patent-claims.py`, "outside as read", never "clear", not legal advice, the sweep stays incomplete). Until then only constraint 8's patent was read, above.

## 11. Open questions

For **CireSnave**: (1) who holds the send grant to `alarmant` (proposal: the PM and cluster PMs); (2) the limits in section 2 (1 open and 3 a day per sender; 3 open and 10 a day overall); (3) whether silence needs an explicit `[UNACKED]` (I recommend no); (4) whether you want quiet hours, knowing they need a patent reading first and that a real emergency would then wait until morning.

For **pm-ants and the synapse lane**: (5) which `synapsed-client` version to build on (rc.123 published, rc.262 in the repo); (6) the `[PAGE v1]` header under MESSAGE-FORMATS, and whether the daemon's header check accepts a new type; (7) how a scheduled send is attributed (section 6); (8) who creates the `alarmant` role's grants and narrows them to the allowed senders.

## 12. Build slices (after CireSnave reads this and the reading is done)

1. The reading (section 10) and the dependency decision (section 4). No code before both.
2. Server: `GET /alert/:id` with tests first (wrong key, unknown id, an acked alert, an open alert, no extra fields); phone server untouched otherwise.
3. Bridge core, pure: page parser (closed header, no text inspection), policy and limits, reply builder. Tests first: every refusal code, an unknown key, a body that mentions another role changes nothing.
4. Bridge binary: claim, read mail, call `POST /alert`, poll, send replies; against a fake daemon and a fake server. `superseded` stops it.
5. Audit file. README rule for senders.
6. One supervised live run to a test role with a test phone, on the PM's word. Nothing has run live until then.

## 13. Not verified

No daemon was contacted and no mail sent. The `synapsed-client` API was read at rc.262 in the repo, not at the published rc.123. The fetch and ack paths and the closed-header acceptance are assumed. The limits have no measurement behind them. The only patent text read for this note is US9686398B2.

— lane: alarmant
