# A5 against the Masimo family: a reading for a person to confirm

Date: 2026-11-01. Lane: AlarmAnt, Sonnet 5.5. **Not legal advice. Not a freedom-to-operate opinion. No clearance.** This is a mapping of claim elements to our planned design, made by a model from Google's machine-converted claim text, for CireSnave (or someone he names) to confirm. Wording is "outside as read" or "reads on". Nothing in A5 was changed to avoid a claim.

Sources: claim text verbatim from `python C:\Projects\tools\patent-claims.py <number>` (Google Patents); family list and statuses from the Google Patents page of US10832818B2 and of US12646617B2 (the newest member), which agree. Statuses and expiry dates are Google's estimates, not the USPTO record. Our design: `DESIGN-A5.md` and `src/lib.rs` on main (the state machine is merged since 0.0.6, 2026-10-29: pure logic, no timers, no provider, no HTTP).

## 1. Our planned escalation (A5), as built

1. An alert is raised by another project (A9: title and text) and pushed to person 1's phone (A2).
2. The phone sounds until Acknowledge. The only thing the phone ever tells the sender is that acknowledgement (A4, `POST /ack/{id}`).
3. No acknowledgement within N minutes: text person 2. N more minutes: call person 2. N more: record `unresolved`. An acknowledgement from anyone ends the alert at any point.
4. There is no "received", "viewed" or "declined" signal from the phone, no urgency levels, no patient or medical element.

## 2. The family (7 members, all shown "Active" by Google, all granted: B2)

| member | relation (Google) | filed | est. expiry | independent claims read verbatim |
|---|---|---|---|---|
| US20150106121A1 (published application of US14/511,972) | parent, published 2015-04-16 | 2014-10-10 | n/a, granted as the next row | 1, 7, 22 |
| US10832818B2 | parent grant | 2014-10-10 | 2037-06-08 | 1 |
| US10825568B2 | division (US15/927,887) | 2018-03-21 | 2034-12-09 | 1 |
| US11488711B2 | continuation (US17/035,382) | 2020-09-28 | 2035-05-31 | 1 |
| US11699526B2 | US17/952,793 | 2022-09-26 | 2034-10-10 | 1 |
| US12009098B2 | US18/323,001 | 2023-05-24 | 2037-06-08 | 1 |
| US12230396B2 | US18/651,438 | 2024-04-30 | 2037-06-08 | 1, 14 |
| US12646617B2 | US19/020,852 | 2025-01-14 | 2037-06-08 | 1, 10 |

Priority 2013-10-11 (provisional 61/890,076). The newest member's page lists the same 7 applications and no child applications. **What this does not show:** a continuation filed after that page was last updated, or any application not yet published (18 months), would not appear. Not checked on the USPTO record.

## 3. Published application claim 1 (US20150106121A1), element by element

| claim 1 element (quoted) | in A5? | note |
|---|---|---|
| "receiving an alarm from a patient device, the alarm comprising information about a physiological parameter status of a patient" | **no** | our alert comes from another project's POST /alert with a title and text; no patient device, no physiological parameter |
| "transmitting an alarm notification to a clinician device in response to receiving the alarm" | in function; "clinician" no | we push to person 1's phone; person 1 is any recipient, not described as a clinician |
| "determining whether an escalation condition is present, the escalation condition depending on one or both of (1) whether a response from the clinician device regarding the alarm notification has been received and (2) content of any response" | **yes in function** | the timer elapsing without an acknowledgement is an escalation condition that depends on whether a response arrived |
| "automatically escalating the alarm by sending the alarm notification to a second clinician device" | **yes in function; "clinician" no** | we text then call person 2 |

Reading: **reads on in function for the last two elements; outside as read on the first**, and on "clinician" if that word is given weight. It is the closest escalation claim found in the whole sweep, which is why it is raised. It is a published application claim; the family was granted with different claims (below). A person should decide whether "an alarm from a patient device ... physiological parameter" can be read to cover an alert from any source: as written it cannot, to this reader.

## 4. The granted claims: what every one of them needs and A5 lacks

Every independent claim of every granted member (10832818 cl. 1; 10825568 cl. 1; 11488711 cl. 1; 11699526 cl. 1; 12009098 cl. 1; 12230396 cl. 1 and 14; 12646617 cl. 1 and 10) requires, from the verbatim text:

| required element | where | in A5? |
|---|---|---|
| a **first indication that the alarm notification was received** at the clinician/user device, sent from the device to the monitoring computer | all granted members | **no**: the phone sends nothing on receipt |
| a **second indication that the notification was viewed but not yet responded to** | all granted members | **no** |
| a response, from a user input, that the user **has declined handling** the alarm | all granted members | **no**: Acknowledge means "I have it", the opposite of declining |
| escalation that is "accelerated" (or "enabled", for the device-side claims) in response to missing or declined indications | all granted members | **no**: A5 escalates only on a fixed timer, and never sooner |
| a patient monitor / patient monitoring computing device / patient device | every independent claim except US12646617B2 claim 10, which says only "physical computer hardware" and "user device" | **no** (claim 10 names no patient element at all; it stands or falls on the three indications) |
| (10832818 only) optical scan of a patient bracelet and a monitor tag, an admit interface with a primary and a secondary clinician, a physiological sensor | 10832818 cl. 1 | **no** |
| (11488711, 11699526) alarm "cleared" determination and escalation to "a second clinician device associated with a second clinician" | cl. 1 | partly: we have a second recipient, no clearance step |

The broadest granted wording is US12646617B2 claims 1 and 10: send a notification to a user device; if a first indication (received) arrives, check for a second indication (viewed, not responded); if that arrives, check for a declined-handling response; on that, accelerate escalation. Claim 10 drops the patient monitoring wording entirely, so it is the claim to put in front of the confirming person. It still needs all three indications from the device. A5 has none of them. **Outside as read, for every granted member.**

## 5. Where the reading could change

- If A3/A4 ever report "received" or "viewed" to the sender, or add a Decline control that escalates sooner, A5 would gain the elements the granted claims turn on. Not planned; the current design has Acknowledge only.
- A continuation not yet visible (point 2) could carry broader wording, as the published claim 1 did.
- "Clinician device" and "patient device" are the words doing the work against the published claim and the granted claims' monitoring computer; a reader who gives them no weight should re-run section 3.
- US5748884A (MCI, 1996, "Expired - Lifetime" per Google; independent claims 1, 11, 18, 19, 20, 27, 28, 30, 33 read verbatim): claims 1, 28, 30 and 33 recite escalating a notification to a default recipient when the first recipient does not confirm within a predetermined time, but each is a mainframe job-processing system with traps, global variables and data control tables, and the patent is expired by Google's field. Outside as read; not confirmed expired on the USPTO record.

## 6. What a person should confirm

1. Section 3: whether the first claim-1 element can be read on an alert from any source.
2. Section 2: that no later or unpublished member exists (USPTO assignment/continuity record, which this lane cannot reach).
3. Whether to accept the section 4 reading for A5 as the sweep's A5 gate.

A5's code (pure state machine, merged 0.0.6) exists on main since before this reading; it sends nothing and no provider is wired. Nothing further is being built on A5 until the PM says so.
