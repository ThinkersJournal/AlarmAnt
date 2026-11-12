# The three-channel escalation and the PC alarm against US 2026/0360532 A1

Date: 2026-11-12. Lane: AlarmAnt, Sonnet 5.5. **Not legal advice. Not a freedom-to-operate opinion. No clearance. No attorney.** A model's reading of claim elements against a design, for CireSnave (or someone he names) to confirm. Read together with `PATENT-MASIMO-19-662313.md` (claim 19 and dependents 20 to 38 against the code as built) and `DESIGN-A5.md` ("Ruling 389").

Design read (CireSnave 2026-11-12, board 389, via the PM): app alarm to the first contact; app alarm to the second contact; an alarm on the operator's PC if neither app alarm is acknowledged. Each step on the one fixed N-minute timer; an Acknowledge from anyone, from any of the three, ends the alert. No SMS or voice for now (kept behind the `AlertChannel` trait).

## Verdict

**Outside as read, and no claim element is present, so this is not an [ASK].** Claim 19 still needs a reply from the notification component that the provider "has not accepted or is unavailable", and escalation "in response to the reply". None of the three channels sends such a reply; every step is a timer on a missing Acknowledge. The PC alarm adds a third endpoint, not a new trigger. Three things to keep true are written as constraints 17 to 22 below.

## Claim 19 against the new design

| claim 19 element | three-channel design |
|---|---|
| 19a patient device / multi-patient monitoring computing device sends the notification | **no**: the sender is the AlarmAnt server, fed by another project's `POST /alert` |
| 19b clinically actionable alarm, physiological parameter over a threshold | **no**: title and text only; nothing clinical anywhere in code or docs |
| 19c notification to a component in a care provider's mobile device | in function for the two app pushes; "care provider to a patient" **no** |
| 19d/19e a reply, from a user input, that the provider "has not accepted or is unavailable" | **no**: the only phone input is Acknowledge (means "I have it"). No decline, snooze, busy or unavailable control, on either phone or on the PC |
| 19f "in response to the reply, immediately escalate ... to a second care provider" | the second **app push** reaches a second person, in function; **trigger no** (timer on a missing Acknowledge, never on a reply, never "immediately"); the PC step is a third endpoint on the same timer |

**Does the escalation element still read?** The same answer as in `PATENT-MASIMO-19-662313.md`: reaching a second person reads in function; the claim's trigger does not. Adding a second app push changes nothing here: claim 19 says "second care provider", not "second device" (`DESIGN-A5.md` Question 2026-11-10, now settled by the ruling).

## The PC alarm, element by element

What it is: a Windows program in the operator's logged-in user session (not a service), which unmutes, sets master volume to maximum through Core Audio and sounds a siren when the escalator reaches the PC step. It receives the instruction by asking the server (see "How it is fired"), and is stopped by the operator.

| question | answer |
|---|---|
| Is the PC "the mobile device of the first care provider" (19c, 19d)? | **No.** It is a third endpoint reached only after two app alarms went unacknowledged; it is not the device that receives the first notification and it sends no reply of the kind 19d describes |
| Claim 27: "the mobile device is a clinician device or a nurse's station system" | Claim 27 narrows the **mobile device** of claim 19. A home PC is not a mobile device, has no clinician or nurse's-station role, and claim 27 inherits the missing 19e. **Outside as read.** If someone argued a PC that sounds an alarm is a "nurse's station system", the claim would still need the negative reply from that device |
| Claims 20 and 21 (patient device routes through or sends directly) | no patient device |
| Claims 22 (whitelisted-apps device) | the PC runs ordinary software; no application whitelisting or kiosk mode (constraint 14) |
| Claims 23 and 24 (received / viewed indications that pause escalation) | **See "How it is fired": the PC's request to the server must not be used as a "received" signal.** The PC step is the last step, so there is nothing left to pause; even so the server must not store or act on the fact that the PC fetched the instruction (constraint 17) |
| Claims 25 to 35 (trend data, video, scanner, admit, EMR) | no |
| Claims 36 to 38 (provider groups, location rules, closest clinician) | no: one flat chain (first contact, second contact, PC); no groups, no location |

## How it is fired, and why that matters for claims 23 and 24

The VPS cannot reach into a home PC, so the PC program must **ask**: a plain authenticated request to the sender's server ("is there an alert at the PC step?"), and sound when the answer says yes. Two rules keep it away from claims 23 and 24 and from the Masimo granted claims' "received" indication:

- **Constraint 17.** The answer to the PC's request is computed from the alert's state alone. The server never records that the PC asked, fetched, displayed or sounded, no state or audit event says "delivered to PC", and nothing is paused, delayed or accelerated because of the request. The request is a poll; it is not a report.
- **Constraint 18.** The PC sends no negative status ("muted", "can't sound", "audio device missing", "user away", "dismissed without handling"). If the PC cannot sound (asleep, off, no output), that is invisible to the server, and the alert goes `unresolved` on its timer like any other. An operator-facing local message on the PC itself is fine; a message to the server is not.
- The PC's one outgoing message is **Acknowledge**, the same as the phones', sent when the operator stops the alarm on purpose. It ends the alert for everyone.

## Windows audio override: patents read for the PC alarm (2026-11-12)

Route: two `WebSearch` queries restricted to patents.google.com (positive control: both returned patent pages, among them US8364221B2 and US8648706B2, which this sweep already holds, so the route reaches the patents and the queries ask about escalating alarms and mute override), then every independent claim of the four closest read verbatim with `python C:\Projects	ools\patent-claims.py` (Google's machine-converted text; statuses are Google's, not the official record). Held hits that also bear on a PC that unmutes and raises volume: US9350851B1 (constraint 2), US8576828B1 (constraint 1), Kyocera US7606561B2 and US8478242B2 (voice call first, then a server override code).

| patent | assignee, status (Google) | independents read | what every independent needs | PC alarm |
|---|---|---|---|---|
| US8855601B2 | Lookout; active est. 2029-12-20 | 1, 16, 26 | a server decides a portable device is **missing** from location data past a distance threshold, then commands an audio transmission (dependents: override a listed setting) | outside; no location, no missing-device test, no audio transmission |
| US8868037B2 | IBM; expired (fee) est. 2032-10-12 | 1, 5 | a **mobile communications device** that verifies a message **signed** with a server's public key, extracts notification parameters and a **digital audio file** from the message's metadata, overrides notification settings; claim 5 adds a second control message after no user interaction in a set period | outside; the PC is not a mobile device, our message carries no signed parameters or audio file, the siren and the volume are local to the program. Closest in shape (escalating override after no interaction): constraint 20 |
| US9148105B2 | Lenovo; active est. 2032-09-02 | 1, 9, 14 | an audio request arrives **while the machine is muted**, its source is **compared with a list of un-mute sources**, played if listed, and **all listed sources un-muted on shutdown** | outside; no list or profile of sources, no per-source comparison, no un-mute on shutdown (constraint 21) |
| US7911983B2 | Cisco; active est. 2029-03-23 | 1, 6, 8, 10 | a GUI for **per-application mute**, plus a second setting that **overrides the mute on an IP telephone call** | outside; no per-application mute interface, no telephone-call trigger (constraint 22) |

Seen in the results and **not read** (titles only; none looked closer than the four above, but nothing is cleared): US8620653B2 (mute control in audio endpoints), US20140372109A1 (smart volume from received audio input), EP3270361B1 (dynamic volume adjustment: raises a security panel's speaker volume toward a maximum when nobody interacts), US11812229B2 (patient monitor alarm speaker analyzer), CN102455888B (ringtone alert exempt from master volume), US9560208B2, US9215543B2, US5987106A. EP3270361B1 and US8364221B2 (already held, "a second speaker sounds when the first alarm is unacknowledged", pulse oximeter with several speakers) are the closest in idea to "louder if unacknowledged"; the PC alarm has no security panel or built-in second speaker, and sounds at one fixed maximum. **A page of web results is not a patent-database search:** this does not replace the Espacenet, WIPO and USPTO routes the sweep still lacks.

## Constraints added

17. The server never records, acts on or exposes that the PC (or a phone) fetched, received, displayed or sounded an alert. The PC step is driven by the alert's state only.
18. The PC sends no negative status to the server; its only outgoing message is Acknowledge.
19. The PC alarm is an endpoint, never a source: it creates no alerts and its request is not a reply to a notification.
20. The PC's volume, unmute and siren settings are local to the program: the server's answer carries no override parameters, no audio file and no signature that the program unwraps (US8868037B2).
21. No list or profile of "un-mute sources" and no per-source match; no un-mute-on-shutdown step (US9148105B2).
22. No per-application mute interface and no override triggered by a telephone call (US7911983B2).

Constraints 1 to 16 stand, including 13 (no negative phone reply, the escalator never acts on a phone message), 14 (no kiosk mode), 15 (no groups or location) and 10 (see below).

## Constraint 10 and the same-operator question

CA 2838076 (BlackBerry) and US9137291B2 (Malikie) turn on devices of **one user** with a stored primary-device indicator, a delay timer on the non-primary device and an acknowledgement that silences the other. In the three-channel design the operator's own phone and the PC may belong to one person. What differs, as read: every timer and every decision is on the **server**; no device holds a primary indicator or a delay timer; an acknowledgement is a message to the server, which ends the alert, rather than a message from one device that silences another. Keep it so: no device-to-device acknowledgement, no device-side delay, no "primary phone" setting. A person should confirm this reading; it is the closest active family to the design after the Masimo claims.

## Where this could change

- The claims of 19/662,313 are pending and can be amended; re-read on any later publication or grant.
- The audio-override route is Google Patents only; eight titles above are unread.
- Building proceeds under the PM's task of 2026-11-12; a person's confirmation of this and the other Masimo readings is still owed. The sweep stays incomplete ("outside as read", never "clear").
