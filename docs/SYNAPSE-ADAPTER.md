# AlarmAnt's part of the Synapse messaging redesign: a human delivery adapter and a watchdog

Status: **DESIGN NOTE ONLY. No code until CireSnave has read it.** Written 2026-11-20 by the AlarmAnt lane at `origin/main` (0.2.11, with #72 pending as 0.3.0). It **replaces** `docs/SYNAPSE-CHANNEL.md`, which pm-ants superseded twice (alarm storage, then agent steps; both gone). Source: `SYNAPSE-MESSAGING-BRIEF.md` (CireSnave 2026-11-19, approved to build), read in full. Nothing here was run: no daemon contacted, no mail sent. *Read* = read from a file or registry page at the ref given; *assumed* = not verified.

## 0. Scope, and what AlarmAnt does not do

From the brief: AlarmAnt is (1) the **human delivery adapter** for urgency-high mail to a human account (full-screen, max-volume, repeat until acknowledged; the acknowledgement comes back as Synapse mail) and (2) the **out-of-band watchdog** (*"Synapse cannot be its own only watchdog"*). **It holds no alarm storage, is not in the agent-alarm path, and has no escalation chain or agent step of its own for Synapse mail**: the clock, failover and agent routing are Synapse's. AlarmAnt's existing human chain (first app, second app, PC alarm; `escalate.rs`) is unchanged and is the behaviour of any alert it raises (section 3.5).

## 1. Where it runs

Read: the phone server is on a VPS (`DESIGN-REGISTRATION.md` section 2); `synapsed` is "one per user, on loopback" (published crate description). So the adapter is a **bridge process on the machine that runs `synapsed`**, a second binary in this crate (proposed name `alarmant-bridge`, as `alarmant-pc` runs on the PC). It claims the Synapse role `alarmant`, reads mail addressed to it, and calls the **existing** `POST /alert` on the phone server over https with the existing API key. It has **no listening socket**. The same process is the PC-side half of the watchdog (section 5).

## 2. The Synapse client

Read on crates.io (2026-11-19): **`synapsed-client` 6.0.0-rc.123** (MIT OR Apache-2.0, rust-version 1.95, updated 2026-10-21), the blocking client for `synapsed`: finds and proves the daemon, claims a role, keeps one session. Written with the `d`: a different crate `synapse-client` (krypticmouse) is **not** ours. No path dependencies. Open before code:

* **The published client lags the repo**: the synapse repo's `origin/main` is rc.262, crates.io has rc.123, and the brief changes the protocol (ack/nack, urgency, modes). The bridge needs the published client that carries the new semantics. **Blocked on the synapse lane's publish, not on design**; every dependency must be at its latest version.
* **Weight and licences** (`synapse` crate, `synapse-security`, blocking `reqwest`, `ring`, `chrono`): `cargo tree` and `cargo deny` not yet run.

The client is blocking, so the async bridge uses `spawn_blocking`. A send carries a caller-chosen `message_id`, and a repeat of the same id is a duplicate (tool schema, read), so every reply AlarmAnt sends uses an id derived from the alert id and a retry after a crash is harmless. One holder per role: if someone claims `alarmant`, the bridge gets 409 `superseded` and **stops and logs, never re-claims** (the client documents the same rule).

## 3. Part 1: the human delivery adapter

### 3.1 What AlarmAnt needs from the message (the format to agree through pm-ants)

AlarmAnt does not define the inbound message; Synapse routes it. It needs these envelope fields, all supplied by the daemon, none read from free text:

    from=<authenticated sender role>   to=<human account>   urgency=high
    message_id=<id>   subject=<at most 100 characters>   body=<at most 1000 characters>

The bridge accepts a message only if **`urgency` is `high`, `to` is a human account it serves (today `ciresnave`), and the daemon authenticated the sender**; anything else is refused with a code. The alert title is `<from>: <subject>` (the sender named by the daemon, so it cannot be dressed as someone else) and the body is the message body with control characters stripped. **The text is never inspected**: nothing in it changes what the bridge does. **No secrets**: the text crosses Google's push service and lands on a lock screen (read: `fcm.rs`).

**As built (2026-11-24, source read by pm-ants):** the daemon's `/v1/fetch` returns per message only `message_id`, `from`, `from_summary`, `from_cwd`, `body_b64`, `enqueued_at`, `lease_until`, `attempts` and `mode`: **no urgency and no addressee field**. So only `from` is the daemon's; `urgency` and `to` are `key=value` pairs in the sender's L1 header (`[TAG] subject to=ciresnave urgency=high`), declared by the sender and **untrusted**. `bridge::parse_envelope` reads that header strictly (v1 tag, one plain subject word, `to` and `urgency` once each, every token `key=value`) and refuses anything else as `malformed`, never echoing it. The subject is the header's subject word and the body is the lines after it. `/v1/ack` acks only as the calling role, so the 3.2 fallback mail is the design until Synapse builds an ack on the human's behalf; the swap is behind `reply_acked`. The `no-device` and `server-down` codes below are **not sent** (they would be delivery reports, S2): a server that is down leaves the message unacked and the daemon redelivers it.

### 3.2 What AlarmAnt sends back (draft, to be agreed with pm-platform through pm-ants)

Preferred, **only if the synapse lane finds it feasible** (pm-platform asked it): the bridge **acknowledges the original message on the human account's behalf** (a Synapse `ack` of `message_id`), so the sender sees an ordinary Synapse ack. This fits **only as a narrow grant**: `alarmant` may ack only messages addressed to the human account with `urgency=high`, every such ack audited with `via=alarmant`, and **never a general act-as**. **Until the synapse lane says that is feasible, the fallback mail below is the design.**

Fallback, ordinary mail from `alarmant` to the original sender, in MESSAGE-FORMATS style: the header is a tag, **one bare subject word**, then `key=value` pairs whose values contain no spaces, and **the tag must be one of the v1 types** (STATUS, READY, MERGED, HOLD, ASK, TASK, NOTICE, FINDING, FIX). The earlier draft of this note used new tags `[ACKED v1]` and `[REFUSED v1]`; **they failed the daemon's check** (see below), so the replies are `[NOTICE]` with the topic as the subject word and the free text on `note:` lines:

    [NOTICE] page-acked affects=<original sender> when=<UTC> action=none
    note: id=<message_id> by=<human account>. Original subject: <at most 100 characters>

    [NOTICE] page-refused affects=<original sender> when=<UTC> action=none
    note: id=<message_id> code=<not-human|not-high|no-grant|sender-limit|open-limit|day-limit|too-long|no-device|server-down>

**L1 check, run 2026-11-20 against the live daemon** by sending each example through the Synapse send tool with `mode` `L1:message-formats/1` (to this lane's own role, so no other role received them): **the first draft, `[ACKED v1] page ...` and `[REFUSED v1] page ...`, 0 of 2 valid** (`not_message_format: [TYPE] is not a v1 type`); **the two `[NOTICE]` forms above, 2 of 2 valid**. What was not checked: the inbound envelope in 3.1 (the daemon, not this lane, defines it), and the real message with a real subject and ids (the examples used a made-up id and subject). The two accepted messages were delivered to this lane's own mailbox and acknowledged.

Refusals say what, never echo the text. A page the human never acknowledges produces **no** message (silence): Synapse's own timeout and failover are what the sender sees (section 3.5).

### 3.3 Acknowledgement and nack (CireSnave's ruling 1) against AlarmAnt's patent constraints

Ruling 1: *"Acknowledgement should always mean 'seen and being dealt with'... every place we can ack we should also be able to nack"*. AlarmAnt's Acknowledge button already means that: the human taps it and the alert ends. Two consequences:

* **Receipt is Synapse's, never AlarmAnt's.** AlarmAnt does not read, record or forward "delivered", "seen" or any phone or PC fetch (PATENT-SWEEP constraint 13: the server never records a phone or PC fetch). The only thing that goes back is the Acknowledge.
* **Stated departure from CireSnave's ruling (PM 2026-11-20): no nack on the phone for now.** *"Every place we can ack we should also be able to nack"* is kept everywhere except the phone, because a phone nack conflicts with a standing constraint. The PM tells CireSnave this departs from his words, and **he may overrule it**; a phone nack waits for a read of US 2026/0360532 claim 19. A nack comes from the terminal client or a Synapse reply instead. The reason, in full:
* **A nack on the phone conflicts with a standing constraint, and I will not add one without a decision.** PATENT-SWEEP constraint 13 says *the phone never sends a negative reply (not accepted, unavailable, busy, snooze) and the escalator never acts on a phone message* (US 2026/0360532 claim 19), and the PM's rule in HANDOFF repeats it: no Decline, no negative status. A "decline, pass it on" button is exactly that. **Ruled: no nack on the phone.** A human who cannot take it ignores it; Synapse's failover (timeout) forwards it, and a deliberate decline is made from the terminal client or a Synapse reply, which is Synapse's own surface, not AlarmAnt's. **If CireSnave overrules, a fresh patent reading of that claim comes first** (question 10.1).

### 3.4 Who may page a human, and how often

The brief gives the receiver the cap (*"each RECEIVER's policy caps them (reminder count, which urgency may page a human, quiet hours, who may page at all)"*). So **the policy lives in Synapse** (who may send urgency-high to a human account, quiet hours, urgency permission). AlarmAnt has one priority by design (constraint 12: no urgency levels) and **must not grow its own quiet hours** (the nearest constraint, 7, concerns a repeating alert gated by schedule periods; I have not read a claim against a plain suppression window, so Synapse's policy stands, not ours). AlarmAnt's job is a **backstop that works even if Synapse's policy is wrong or bypassed**, and it covers a gap I found:

* *Read:* there is **no rate or count limit on `POST /alert`** in `raise.rs`, `serve.rs` or `routes.rs` (the only limiter is the failed-attempt one on `/register`). Any holder of an API key can raise alerts without bound, and the bridge would inherit that.
* **This is a defect in existing code, not part of this design, and it is fixed on its own:** ROADMAP row A10 (opened 2026-11-20) schedules a per-key cap in the next code PR after #72, with a red-first test (the N+1th alert inside the window is refused with a fixed message, and a second key of the same sender cannot bypass it). **Until that PR merges the existing endpoint is unlimited**, whatever this note says about limits; the bridge's own per-sender caps below are additional, not a substitute.
* **Proposal (starting values; CireSnave rules on the numbers):** per API key 1 open alert, 6 raised an hour, 20 a day; overall 3 open; **per sender role (the bridge enforces it from the daemon's authenticated `from`)** 1 open and 3 a day. Over a limit: refused at once with a code, never queued, and audited. The 3 am case is covered by the grant, Synapse's receiver policy, these caps and the named sender on the alert; a sender that stays inside every limit and holds the grant can still page about a status, and only the sender rule (below) discourages it.
* **Rule for senders** (README and the grant message): page a human only when a decision or action by CireSnave is needed and cannot wait for the next time he reads his board.

### 3.5 Chain, reminders and Synapse failover do not double up

The adapter raises an ordinary alert, so AlarmAnt's chain (first app, second app after N minutes, PC alarm after 2N) and the phone's repeat-until-acknowledged run as today. The brief's failover (an un-acked message forwarded to the next receiver after a settable time) is Synapse's and runs on its own clock. **They can overlap**: Synapse could forward to a second receiver while AlarmAnt is still pushing to the second app. That is harmless but may be surprising. **Question 10.2:** should the failover timeout for a human-account message be longer than 3N (so AlarmAnt's own chain finishes first), or is a parallel forward wanted? Recommendation, which pm-platform's read of the brief agrees with: **the failover timeout for human-account mail is longer than 3N by default, and the sender may shorten it** (that is Synapse policy, set on the Synapse side).

### 3.6 The acknowledgement path

1. The bridge raises the alert with `POST /alert`, receives `{"alert_id"}` (read: `raise.rs`), and keeps `message_id` against it (in memory plus a small file, so a restart still knows which alerts are open).
2. The phone or the PC alarm acknowledges to the server as today. **Nothing changes there.**
3. The bridge asks the server through a new read-only **`GET /alert/:id`** with the same API key, answering `{"state":"open"}` or `{"state":"acked"}` **and nothing else** (not the chain step, not any fetch). It polls each open alert (proposal: every 20 seconds, for at most 24 hours), then sends the acknowledgement (3.2) once. This reads state the server already holds (`Acked`, used by the PC board today); it needs the key check, a fixed-shape answer and tests first (wrong key; unknown id answers 404 with no hint; acked and open alerts).

## 4. Threat model for Part 1

| Threat | Mitigation |
|---|---|
| A lane pages the human for a status | Synapse receiver policy and grant (the brief's gate); AlarmAnt's per-sender, per-key and overall caps; the sender named on the alert; refusals audited |
| Impersonation | the sender is the daemon's authenticated envelope role; nothing in the body is believed; the title is written by the bridge |
| Text carries an injection or a secret | text is data, never inspected or acted on; length caps; control characters stripped; senders told no secrets; the audit stores a length and SHA-256, never the text |
| Page storm | one open alert per sender and per key, daily caps, refusal not queueing |
| The bridge's API key leaks | read from a file at start (never argv, env dump or log), like the admin key; it can only raise alerts and read one alert's open/acked state, never register a device (`DESIGN-REGISTRATION.md` section 3) |
| Someone claims the `alarmant` role | stop and log on `superseded`; the watchdog (section 5) sees the bridge go quiet |
| A compromised bridge | its Synapse grants are least privilege (receive as `alarmant`; send only the acknowledgement and refusal mail; ack as the human endpoint only if granted); its server credential can only raise alerts and read open/acked |

**Audit** (append-only, owner-only file, one JSON line each, names and codes only): `received`, `raised`, `acked`, `refused`, `superseded`, `started`, with time, authenticated sender, alert id, message id, refusal code, and for a received message the text length and SHA-256. Never: the API key, an Authorization header, the Synapse session, the text.

## 5. Part 2: the independent watchdog

**Purpose:** Synapse cannot page anyone when Synapse is the thing that is down, so AlarmAnt keeps a path that does not go through it. Three observers, because each one's failure must be caught by another:

| Observer | Runs on | Watches | If it sees a failure |
|---|---|---|---|
| **A: server-side dead-man** | the VPS (the phone server) | a **heartbeat** from the bridge | pages the phone **directly** through the normal alert path (Synapse and the PC are not involved) |
| **B: PC-side check** | the bridge on the Synapse machine | the VPS server (`GET /health` answering fixed text) | sounds a local siren through `alarmant-pc` and writes a terminal notice; it cannot reach the phone without the server |
| **C: an external dead-man** | a third-party heartbeat service (PM ruling 2026-11-20: **in the design**) | pings from the VPS watchdog and from the bridge | notices when A **and** the VPS, or the PC and the VPS, are down together, and pages **CireSnave's own phone with no AlarmAnt dependency** (section 5.6). Creating the account is CireSnave's, not the lane's |

### 5.1 What the bridge reports and how often

Every 60 seconds the bridge posts a **heartbeat** to the server (`POST /heartbeat`, a new route with its **own key**, which can post heartbeats and nothing else, never the API key): fixed fields `{"synapse":"up|down","canary":"ok|late","bridge":"up"}`. How it knows: `synapsed-client` proves the daemon before any credential is sent (read, rc.262), then the bridge runs a **canary**: every 5 minutes a message from `alarmant` to itself (needs a send grant to self) must come back within 30 seconds, proving mail is delivered end to end, not only that the port answers. The heartbeat carries no secret and no text.

**What the PC is, precisely:** "the PC" is the machine that hosts `synapsed`, the bridge and the lanes. Its liveness is the bridge's heartbeat. **`alarmant-pc`'s polls of `/pc/alarm` are never used as a liveness signal**: that would make the server record a PC fetch, which constraint 13 forbids, and the server is never told when the PC could not sound (constraint 18). The watchdog is a separate process with a separate route.

### 5.2 What triggers a page, and how a false-alarm storm is avoided

* **Observer A pages** when **3 consecutive heartbeats are missing** (at 60 seconds, about 3 minutes), or when a heartbeat says `synapse: down` or `canary: late` **3 times in a row**. A single miss or late canary pages nothing.
* **Clearing needs 3 consecutive good heartbeats**, so a flapping link does not page, clear and page again.
* **One open watchdog alert per component** (`pc`, `synapse`) at a time, and **no new page for a component for 30 minutes after a human acknowledges it** even if the fault persists (the human has been told; a second page would be the storm). Overall cap: **2 watchdog pages an hour**. A page over a cap is audited and not sent.
* **Maintenance:** `alarmant watchdog pause --minutes N` (operator CLI on the VPS, maximum 60, audited, expires by itself) so a planned reboot is not an incident. *Patent note:* a time-bounded manual pause is not a schedule, but it sits near constraint 7; I am flagging it rather than deciding it (question 10.5).
* **After a VPS restart** the watchdog starts with a grace period equal to the missed-beat threshold, so a restart does not read as a dead PC.

### 5.3 What the page says

Fixed text, same alert path, same single priority (no levels): title `AlarmAnt watchdog`, body `The PC stopped reporting at <UTC>`, or `Synapse is not delivering mail since <UTC>`, or `Synapse is down since <UTC>`. No secrets, no counts, nothing from the message path. It goes through the normal chain (first app, second app, PC alarm); the PC alarm step is useless when the PC is the fault, which is why the first two matter.

### 5.4 How the watchdog proves it is itself alive

* The phone server already runs a **daily self-test** (A6) that exercises the chain; it stays and is the existing proof of the VPS path. A failed self-test goes to the failure notifier, which is not the phone.
* Observer B proves the VPS is answering every minute; if it is not, the PC sounds and logs locally. Observer A is the VPS's own check of the PC, so each side watches the other.
* **What A and B alone do not cover:** the VPS and the PC down together, or the VPS down while the PC is off. Only observer C covers that, and **until CireSnave has created its account and the pings are configured, a double failure is silent.** C is in the design; its activation is his (section 5.6).
* The watchdog's own failures are audited (heartbeat route errors, key rejected, store unreadable), and a watchdog that cannot start refuses to start, loudly, rather than run blind.

### 5.6 Observer C: the external dead-man

**Shape.** Two **pings out**, both plain HTTPS GETs to a check URL: one from the VPS watchdog every minute, one from the bridge every minute. The service pages **only when pings stop**, which is exactly the one failure the rest cannot see. The URLs are secrets (anyone who has one can keep the check green): kept in files, never argv or logs, never in the repository. It has **no dependency on AlarmAnt, Synapse or the VPS** at the moment it pages, and it must page CireSnave's own phone by a path that is not the alarmant app.

**Two checks, not one**, so the page says which side died: `alarmant-vps` (pinged by the watchdog) and `alarmant-pc` (pinged by the bridge). Grace period about 3 minutes each, matching 5.2.

**Candidates**, from one round of third-party listings **that I could not check against the vendors' own pages** (prices and plans change; the account holder should confirm each on the vendor's site before choosing). What each needs is the same shape: an account, a check URL per check, and the channels below.

| Service | Account and cost (unverified) | How it can page CireSnave's own phone (unverified) |
|---|---|---|
| **Healthchecks.io** (open source; a hosted free tier of about 20 checks is reported; paid tiers reported at $20 and $80 a month) | one account; the free tier looks enough for two checks | email, webhooks, SMS and phone call (some of these reported as paid features), and a **Pushover** integration. **CireSnave already uses Pushover as the stopgap in the ROADMAP**, so a Pushover emergency-priority page (which repeats until acknowledged) is the natural fit if the integration exposes that priority; **not verified** |
| **Better Stack** (heartbeats on a free plan of about 10 monitors are reported) | one account | email and Slack on the free plan are reported; phone call and SMS appear to need paid on-call seats (about $29 a responder a month, reported); **not verified** |
| **Cronitor** (a free tier of about 5 monitors is reported, conflicting listings) | one account | email and Slack reported; SMS reported as paid; phone call reported as not offered; **not verified** |
| **Dead Man's Snitch** (a free plan with one check is reported; paid prices differ between listings) | one account; two checks may need a paid plan | not established |
| **Self-hosted Healthchecks** | a small host that is **neither the VPS nor the PC**; cost and upkeep | the same channels as the hosted one, plus its own upkeep: a second thing to watch |

**No service is chosen here.** It is CireSnave's account and call; the table is only what a choice has to weigh, and every cell is unverified. One observation for him, not a recommendation: the ROADMAP's stopgap is already Pushover emergency priority on his phone, so a service that can page through Pushover would not need the alarmant app at all; whether any listed service can, at which plan, is for him to confirm on the vendor's page. **Whatever he picks, three things must be confirmed before it is relied on:** the exact plan limits, that a check going silent triggers a page to his own phone, and that a test page arrives with the alarmant app uninstalled. A drill with the VPS and the bridge stopped is the proof, with his word, once.

**What C does not do:** it knows only that pings stopped, not why; it carries no message text and no secret.

### 5.5 Watchdog threats

| Threat | Mitigation |
|---|---|
| Fake heartbeats keep a dead PC looking alive | a separate key kept in a file; rate-limited; fixed-shape body; a stolen key is a risk to name, bounded because it can only hold the page off (it cannot raise one or read anything) |
| Fake outage pages | the route is key-checked; pages only follow missing or `down` heartbeats, and caps apply |
| A page storm during a flap | 3-in-a-row to page and to clear, one open per component, a 30-minute quiet after an acknowledgement, 2 an hour overall |
| The watchdog itself hangs | observer B, the daily self-test, and observer C (once its account exists) |
| A check URL leaks | anyone holding it can keep a dead system looking alive; the URLs are secrets kept in files; rotate them at the service if one is exposed |

## 6. Patent claim table and reading

Every current constraint (PATENT-SWEEP 1 to 16, `PATENT-REGISTRATION-READING.md` 1 to 6, `PATENT-TLS-PIN-READING.md` T1 to T5) was written for the phone and PC path, which **this design does not change** (the chain, the one-step Acknowledge, the PC alarm, registration). Against the new surfaces:

| Constraint | Effect |
|---|---|
| 13 (no negative reply; the escalator never acts on a phone message; the server never records a fetch) | kept **by not adding a phone nack** (3.3); the only thing sent back is an Acknowledge; the heartbeat is a separate route and `alarmant-pc`'s polls are not used (5.1). **The one place the brief and the constraint meet is the nack, flagged for CireSnave** |
| 12 (no urgency levels, threshold types, post-acknowledged states) | kept: AlarmAnt reads `urgency=high` only as an entry condition set by Synapse's policy; it adds no level, category or status ladder; the watchdog page is the same single-priority alert |
| 7 (a repeating alert gated by schedule periods; US8825014B2) | no quiet hours in AlarmAnt; the maintenance pause is flagged (5.2) |
| 8 (no alert triggered by detecting text in a message body; US9686398B2) | the closest the adapter comes, so I read it (Verizon; Google says Expired (fee); independents 1, 7, 14): **a user device in a silent mode detects a text prompt in a message body and overrides the silent mode.** Ours: the mail is addressed to a **service role**, accepted by **who sent it, the grant and the daemon-set urgency**, never by scanning text; the phone gets a data push and detects nothing. *Outside as read.* The text-never-inspected rule keeps it so |
| 6, 9, 10, 11, 15, 16 | not touched: one acknowledgement step, one device per recipient, no change of device on a missing ack, no roster, no clinical use |
| T1 to T5 | not touched |

**Is a re-sweep needed before code? A fresh reading, yes; a full re-sweep, no.** The two new surfaces are (a) an inbound machine-to-service channel that raises a human alert and (b) a heartbeat dead-man monitor that pages. Nearby art is likely uptime and heartbeat monitors, dead-man's switches, on-call paging and alert gateways (several are mature products). **I have not run those searches**: this note is docs-only. **No code of either part starts until `PATENT-SYNAPSE-READING.md` is recorded in the repository; I will run it on 2026-11-21 (the day this note's PR opens) and record it by 2026-11-22; a slipped date is reported to pm-ants, not worked around.** Format of the earlier readings: a few searches, independents read with `tools/patent-claims.py`, "outside as read", never "clear", not legal advice, the sweep stays incomplete. Until then only US9686398B2 has been read for this note.

## 7. What the Android app and the phone path gain

Nothing. No new button, no new status, no new screen; the alert is the one it already is. The only server additions are routes with the bridge's keys (`GET /alert/:id`, `POST /heartbeat`, `GET /health`), each with a fixed-shape answer and tests first.

## 8. Build slices (after CireSnave reads this and the reading is recorded)

1. The reading (section 6) and the dependency decision (section 2). No code before both.
2. Server: `GET /alert/:id`, per-key rate limits on `POST /alert`, `GET /health`, `POST /heartbeat` with its own key; tests first (wrong key; unknown id; acked vs open; over-limit refusals; the 3-missed-beats and 3-good-clear logic with a fake clock; one open per component; quiet after an acknowledgement; grace after restart; pause expiry).
3. Bridge core, pure: envelope check, caps, reply builder; tests first (every refusal code; a body that mentions another role changes nothing).
4. Bridge binary: claim, read mail, raise, poll, reply; heartbeat and canary; against a fake daemon and a fake server; `superseded` stops it.
5. Observer B on the PC (local siren and terminal notice).
6. One supervised live run on the PM's word, to a test role and a test phone. Nothing has run live until then.

## 9. Not verified

No daemon was contacted and no mail sent. The `synapsed-client` API was read at rc.262 in the repo, not at the published rc.123; the brief's new semantics (urgency, ack as another account, receiver policy) do not exist in either yet. The fetch and ack paths are assumed from the MCP adapter. The limits and cadences have no measurement behind them. The only patent text read for this note is US9686398B2.

## 10. Open questions

For **CireSnave**: (1) a nack on the phone, no (my recommendation) or yes after a patent reading; (2) the failover timeout for a human-account message longer than 3N, or parallel; (3) the numbers: **provisional defaults, all settable in config**, in the one table in section 11, for him to read with this note; (4) the external dead-man (C) is in the design by ruling; he chooses the service and creates the account (section 5.6), and says when to run the drill that proves it; (5) a maintenance pause, yes or no.

For **pm-ants, pm-platform and the synapse lane**: (6) which `synapsed-client` version carries the new semantics and when it is published; (7) whether the daemon will let role `alarmant` ack as the human endpoint, or the fallback mail format (3.2) stands; (8) the envelope fields in 3.1 under MESSAGE-FORMATS, agreed through pm-ants; (9) a send grant from `alarmant` to itself for the canary; (10) who builds the **human terminal client** that the brief lists under the alarmant lane's step 5: it is outside this note and I have not designed it.

## 11. Provisional defaults (one table; every value is a config setting)

All are my starting values with no measurement behind them; CireSnave reads them with this note and may change any.

| Setting | Default | Section |
|---|---|---|
| `POST /alert` per key owner: raised per hour / per day (built on branch `alert-rate-limit`, ROADMAP A10; `ALARMANT_RATE_PER_HOUR`, `ALARMANT_RATE_PER_DAY`) | 6 / 20 | 3.4, ROADMAP A10 |
| `POST /alert` per key owner: open alerts at once (**proposed here, not in the A10 code**) | 1 | 3.4 |
| Throttle notice to the human: first refusal per owner, then at most one per | 1 hour (exempt from the cap) | 3.4, ROADMAP A10 |
| `POST /alert` overall open | 3 | 3.4 |
| Bridge per sender role: open / per day | 1 / 3 | 3.4 |
| Ack poll interval, and how long an alert is followed | 20 seconds, 24 hours | 3.6 |
| Heartbeat interval (bridge to server, and each external ping) | 60 seconds | 5.1, 5.6 |
| Canary (bridge mails itself), and its allowed delay | every 5 minutes, 30 seconds | 5.1 |
| Missed or bad heartbeats before a page, and good ones before clearing | 3 and 3 | 5.2 |
| Quiet after a human acknowledges a watchdog page | 30 minutes | 5.2 |
| Watchdog pages overall | 2 an hour | 5.2 |
| Maintenance pause, maximum | 60 minutes | 5.2 |
| Grace after a VPS restart | the missed-beat threshold (3 minutes) | 5.2 |
| External check grace period | about 3 minutes | 5.6 |
| Alert title, body and subject lengths | 100 and 1000 characters (existing limits) | 3.1 |

— lane: alarmant
