# Claim chart: US 9235547 / US RE47029 against AlarmAnt (board 490)

Date 2026-11-22. AlarmAnt lane (Sonnet 5.5), for pm-ants TASK `us9235547-claim-chart`. **A model reading, not legal advice, no attorney.** Claims are the Google Patents page text (a third-party copy). Never "clear": the sweep is partial. AlarmAnt side read at `origin/main` 0.4.0 (04010ad): `raise.rs`, `escalate.rs`, `serve.rs`, `channel.rs`, `bridge.rs`, `docs/SYNAPSE-ADAPTER.md`.

## 1. The family changes which claims are live
* US 9235547 B1 (13/374,271, Individual, filed 2011-12-20, priority 2010-12-20) shows **"Ceased"** on Google: it was reissued as **US RE47029 E1 (15/421,589, filed 2017-02-01), Active, est. expiry 2034-09-13** (Google, not official). The live patent is therefore RE47029, 181 claims.
* RE47029 claims 1-80 read word for word like US 9235547 claims 1-80 on Google's text; claims 81-160 are new dependents on 1, 21, 41, 61; **claim 161 is a new independent** (system) adding two limbs (see 4). Google's text carries no reissue brackets or underlines, so which words were amended is not visible; only the USPTO record shows it.
* **USPTO ODP, run 2026-11-22 (Hello approved on the second try):** 13/374,271 Patented Case, patent 9235547 granted 2016-01-12, filed 2011-12-20, parent provisional 61/459,931 (Expired). Its only child is **15/421,589, a Re-Issue, patent RE47029, granted 2018-09-04, filed 2017-02-01, status Patented Case**. No continuation, divisional or other child in the continuity bag. The reissue's claim file of 2018-03-12 (`CLM`, XML) prints **only new claims 81-181** (101 marked New): claims 1-80 were **not amended** in the reissue, so the original 1-80 stand as issued. Claim 161 in that XML equals Google's text word for word apart from the XML's markers and five colon/semicolon differences in punctuation (checked by normalised comparison). **Not done:** claims 1/21 against the original grant XML (the reissue XML does not reprint them) and the expiry date (Google's 2034-09-13 only; not read from USPTO).

## 2. Limb chart, independents 1 (medium), 21 (system), 41 (method), 61 (medium)
Claims 1, 21, 41, 61 share every limb below; differences are noted. P = present, D = different form, A = absent. "Needs a person" marks a limb that could be argued either way.

| # | Limb (claims 1/21/41/61) | AlarmAnt (read) | |
|---|---|---|---|
| a | communications server | the VPS server (`serve.rs`) | P |
| a1 | message database | `AlertStore` / persisted JSON files (`persist.rs`), holds id, state, title/body | P |
| a2 | message file server, message host process operating on it | one server process, JSON files; no file server | D (needs a person) |
| b | remote source interface for data entry of messages | `POST /alert` (API key); Synapse bridge (not wired) | D |
| c | source client process monitoring **acceptance/reading** by target | `GET /alert/:id` returns `open`/`acked` only; server never records a fetch or read (constraint 13) | D: acceptance (Ack) yes, reading no |
| e | remote target interface for receiving, displaying, replying | phone app, PC alarm; reply is Acknowledge only | P |
| f | target client process residing on the communications server | apps reside on the phone/PC; the server only pushes | D |
| g1 | source receives message **and severity/urgency classification** | `POST /alert` takes `title`+`body` only; other fields ignored (`raise.rs` parse_request). The unwired bridge admits only urgency=high but forwards no class | **A** |
| g2 | classification transmitted, **stored**, and sent to the target with the message | not stored, not sent | **A** |
| g3 | target process responds to host **if message is examined** | phone sends Acknowledge only; no "examined/viewed" response (constraint 13). Ack implies examination: needs a person | D (needs a person) |
| g4 | dead man process waits a predetermined time for the response; if exceeded the message is **updated and forwarded to another target interface for another user** (claim 61: "retransmitted") | `escalate.rs`: one fixed N-minute timer, then second app (a second contact), then PC alarm. Forwarded: yes. "Updated": the text is unchanged, only the alert's state/stage moves | **P in substance for forwarding; "updated" is D** |
| g5 | host **reports to source the status of the message to each target** (claim 61: "status of attempts to deliver") | only `open`/`acked`, never the chain step, never per target (serve.rs, constraint 13) | **A** (partial: one aggregate status) |
| s1-3 | originator logs on, enters message, **enters classification** | login by API key; no classification step | **A** (step 3) |
| s4 | store message + class via file server | stored without class | D |
| s5-6 | traverse a **hierarchical target message thread**, select next target | a fixed three-stage chain in code (`Stage`), no list, no thread, no traversal of a configurable thread | D (needs a person: a fixed ordered chain may be argued to be one) |
| s7 | determine if thread exhausted | chain ends at the PC alarm, "no third tier" | P in effect |
| s8-9 | transmit to the selected target; determine whether it has been **read** | pushes; the server never learns of a read | **A** for "read"; Ack only |
| s10 | target response timer | the fixed timer | P |
| s12 | **report to source that message reception has failed**, then next target | **CORRECTED 2026-11-22** (my first chart said A): `POST /alert` answers the sender synchronously with an error when the push to the first device fails (410 `DeviceGone` for an unregistered token, 502 rejected, 503 transient; `raise.rs`), and the alert record is removed. That is a report to the source that the push to the first target failed, but there is no next target after it (no forward) and nothing is reported later on a missing ack (constraint S2). The unwired bridge's `page-refused` codes (`no-device`, `server-down`) restate the same thing. Different: sender told of a failed push at raise time, not of a target's failed reception inside a thread | D (needs a person) |
| s13 | report to source that message **has been received**, return any user comment | `GET /alert/:id` = `acked`; planned `page-acked` NOTICE; no comment | D |
| s14 | optionally enter comments / pass the message to another thread | none: no comment, no user forwarding | A (optional step) |

## 3. Answers asked
* **Does escalate-when-unacknowledged alone meet the dead-man limb (g4)?** Read plainly, yes for the timeout-and-forward part: a fixed timer, then a push to another contact. The "updated" wording is the one soft spot (text unchanged). **AlarmAnt's position does not rest on that limb.**
* **What AlarmAnt lacks:** (1) the severity/urgency classification carried source -> host -> target (limbs g1, g2, s3), **the strongest gap**; (2) per-target status to the source (g5); a later "reception failed" report inside a thread (s12: only the raise-time push error exists); (3) any "read/examined" determination (c, g3, s9); (4) a configurable hierarchical thread with traversal (s5-6: different, arguable); (5) a message file server (a2: different, arguable).
* **Reading:** claims 1, 21, 41, 61: **outside as read (partial search; third-party text; no person has read it)**. Every limb must be present; g1/g2/s3 (classification), g5 (status to each target) and the read determination (c, g3, s9) are absent. s12 is now different, not absent (corrected above), so the classification gap carries the reading most.

## 4. RE47029 claim 161 (the added independent), limb by limb
Claim 161 = claim 21's limbs (so all of section 2 still applies: classification, per-target status, read determination) **plus** the four below, and its steps renumber with two new steps (5, 6) and a new step 13.

| # | New limb in claim 161 | AlarmAnt (read at 0.4.0) | |
|---|---|---|---|
| n1 | a **second dead man process monitoring the communication between the remote target system and the host**; on a **detected loss in communication** the message is updated and forwarded to another target | none. The server never tracks a device's connection or last contact: it does not record a phone or PC fetch (constraint 13), `GET /pc/alarm` is stateless, FCM is fire-and-forget. The chain moves only on the fixed timer (`escalate.rs`: `tick`, no input but the clock). A push failure at raise time ends the request with an error and creates no forward (`raise.rs`) | **A** |
| n2 | host **monitors availability** of the target: logged on, **logged off for non-use**, anomaly | none: no login state or session for a phone or PC; the apps hold a bearer key and a token, nothing is "logged on". Registration state (`register.rs`) records an enrolled device, not availability | **A** |
| n3 | step 5: **detect target availability by polling** the hierarchical thread, proceed if available | none: the server never polls a target. The only polling is **by** the PC program and by the unwired bridge **of the server** (`GET /pc/alarm`, `GET /alert/:id`), the reverse direction | **A** |
| n4 | step 6: **modify the thread, removing an unavailable target**, then back to step 4 | none: the chain is fixed in code; an unregistered token (`TokenGone`) returns 410 to the sender at raise time and removes the alert, it does not edit a thread or reroute | **A** |
| n5 | step 13: determine whether a target is **in communication with the message host**; if so go on to step 8 | none (as n1) | **A** |

**The heartbeat and dead-man contact, read closely.** (i) The **daily self-test** (`selftest.rs`, A6): raises a real alert once a day, and a missing or lost ack is a failure signal handed to a *failure notifier, "not the phone"*; it never forwards the alert to another target and never edits a chain. It is the nearest thing to monitoring a channel's health, and what it does with a failure (report to the operator) is not claim 161's act (update and forward to another target). Keep it that way: **its output must never feed the escalator.** (ii) The **PC alarm program** polls `GET /pc/alarm` and treats an unreachable server as "change nothing" (`pcclient.rs`); that is the client watching the server, again reverse. (iii) The **watchdog / heartbeat** (docs/SYNAPSE-ADAPTER.md section 5) is **not built** (ruling 478); designed as a one-way beat with a fixed deadline per source, pages the same single alert on a miss (constraints S4, S6, S7). A watchdog that pages when a *Synapse role* goes quiet monitors the sender side, not "communication between the remote target and the host", and does not forward an existing message to another target; but a design that paged because the phone or PC stopped polling, or that skipped or removed a target for being offline, would read on n1-n4.

**Reading:** claim 161 **outside as read (partial search; Google text, checked against the USPTO XML for 161 only; no person)**: n1-n5 are all absent and any one absent defeats the claim, in addition to the section 2 gaps. These are the limbs ruling 407-A (slice (h), "FCM push failed, escalate now") already keeps out; this chart is a second reason to hold that line.

## 5. What would change this reading (re-read first)
Adding a severity/urgency/priority field to `/alert` that is stored or sent on; a delivery-failed or per-target status to the sender (including widening the bridge's `page-refused` to "no-device"/"server-down" cases: a person to confirm these codes are admission refusals only); `GET /alert/:id` showing chain step; any read/viewed signal from phone or PC; any escalation triggered by loss of contact or device availability (the watchdog must not feed the escalator); a configurable target list.

## 6. Doubt and owed
Claims 1-80 against the original grant (13/374,271 `CLM`/grant XML) and the expiry date are still unchecked at USPTO; the 161 text was checked. The comparison of claims 1/21 can use the same key approval. A person's confirmation of everything above is owed.
