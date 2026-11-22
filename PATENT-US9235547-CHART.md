# Claim chart: US 9235547 / US RE47029 against AlarmAnt (board 490)

Date 2026-11-22. AlarmAnt lane (Sonnet 5.5), for pm-ants TASK `us9235547-claim-chart`. **A model reading, not legal advice, no attorney.** Claims are the Google Patents page text (a third-party copy). Never "clear": the sweep is partial. AlarmAnt side read at `origin/main` 0.4.0 (04010ad): `raise.rs`, `escalate.rs`, `serve.rs`, `channel.rs`, `bridge.rs`, `docs/SYNAPSE-ADAPTER.md`.

## 1. The family changes which claims are live
* US 9235547 B1 (13/374,271, Individual, filed 2011-12-20, priority 2010-12-20) shows **"Ceased"** on Google: it was reissued as **US RE47029 E1 (15/421,589, filed 2017-02-01), Active, est. expiry 2034-09-13** (Google, not official). The live patent is therefore RE47029, 181 claims.
* RE47029 claims 1-80 read word for word like US 9235547 claims 1-80 on Google's text; claims 81-160 are new dependents on 1, 21, 41, 61; **claim 161 is a new independent** (system) adding two limbs (see 4). Google's text carries no reissue brackets or underlines, so which words were amended is not visible; only the USPTO record shows it.
* **Not done:** the USPTO ODP continuity read (the Windows Hello prompt timed out, 2026-11-22) and the comparison of one claim with the USPTO XML/PDF. Both owed. Other family members: Google lists only these two.

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
| s12 | **report to source that message reception has failed**, then next target | nothing is sent to the sender on a missing ack or failed delivery (constraint S2). The unwired bridge's `page-refused` reply (codes incl. `no-device`, `server-down`) reports a refused **admission/raise**, not a target's failed reception: nearest thing, needs a person | **A** (flag) |
| s13 | report to source that message **has been received**, return any user comment | `GET /alert/:id` = `acked`; planned `page-acked` NOTICE; no comment | D |
| s14 | optionally enter comments / pass the message to another thread | none: no comment, no user forwarding | A (optional step) |

## 3. Answers asked
* **Does escalate-when-unacknowledged alone meet the dead-man limb (g4)?** Read plainly, yes for the timeout-and-forward part: a fixed timer, then a push to another contact. The "updated" wording is the one soft spot (text unchanged). **AlarmAnt's position does not rest on that limb.**
* **What AlarmAnt lacks:** (1) the severity/urgency classification carried source -> host -> target (limbs g1, g2, s3), **the strongest gap**; (2) per-target status and a "reception failed" report to the source (g5, s12); (3) any "read/examined" determination (c, g3, s9); (4) a configurable hierarchical thread with traversal (s5-6: different, arguable); (5) a message file server (a2: different, arguable).
* **Reading:** claims 1, 21, 41, 61: **outside as read (partial search; third-party text; no person has read it)**. Every limb must be present and at least g1/g2/s3, s12 and g5 are absent.

## 4. RE47029 claim 161 (the added independent)
Same limbs plus: a second dead man process that **monitors the communication between target system and host and forwards on a loss of communication**; the host **monitors availability** (logged on, logged off for non-use, anomaly); steps to **poll target availability and remove an unavailable target from the thread**. AlarmAnt: none of these. A "loss of communication" forward is exactly what ruling 407-A refuses (slice (h): reporting "unregistered"/"permissions missing" to the escalator; any "FCM push failed, escalate now" shortcut). Outside as read, and these are the limbs the constraints guard.

## 5. What would change this reading (re-read first)
Adding a severity/urgency/priority field to `/alert` that is stored or sent on; a delivery-failed or per-target status to the sender (including widening the bridge's `page-refused` to "no-device"/"server-down" cases: a person to confirm these codes are admission refusals only); `GET /alert/:id` showing chain step; any read/viewed signal from phone or PC; any escalation triggered by loss of contact or device availability (the watchdog must not feed the escalator); a configurable target list.

## 6. Doubt and owed
Google's text is a third-party copy and a reissue shows no markup there: check one claim against the USPTO XML/PDF and run the ODP continuity (needs one more Hello approval). A person's confirmation of everything above is owed.
