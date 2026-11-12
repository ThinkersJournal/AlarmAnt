# A5 design: escalation to a second person

Status: DESIGN ONLY (ant-first, PM 2026-10-28). No escalation code. Unblocked by the PM on the USPTO reads (2026-10-29); the limits in PATENT-SWEEP.md are read-again triggers, and before the first crates.io publish the 3 unread applications must be read or stated "not read" there with the reason.

## Behaviour
1. An alert is sent to person 1 (A2) and the state is `sent`.
2. If no acknowledgement arrives within N minutes (default 5, one setting), the alert is sent once to person 2 by SMS, then by voice call if N more minutes pass. State `escalated`.
3. An acknowledgement from anyone ends the alert: state `acked`. Person 1's phone keeps alarming until acknowledged; escalation never silences it.
4. If person 2 also does not acknowledge, the alert stays `escalated` and an audit event `alert.unresolved` is written. No third tier.

## Constraints from PATENT-SWEEP.md (1-8) and how this design meets them
- One flat chain: person 1, person 2. No supervisory tiers, no data-source analytics (3).
- One alert priority; no reordering (4).
- One acknowledgement step. No `accepted` state, no timer that runs after acceptance (6).
- The send channel for person 2 is chosen by a fixed rule (SMS then call), never by stored delivery-performance profiles (3).
- Repeat and timeout settings are plain numbers; no battery-level gate and no schedule periods (7).
- The alert is a data push; nothing parses message text (8).
- No wearable or gesture acknowledgement, no auto-pause by history (5).

## State machine (AlarmAnt's own code)
`sent -> acked` | `sent -> escalated -> acked` | `escalated -> unresolved`. Transitions are written to PersistAnt and audited with AuditAnt.

## Shared Ants needed (stubs only; only ConfigurAnt and PersistAnt are real releases today)
| Need | Ant | Stub |
|---|---|---|
| N, contact list | ConfigurAnt | `EscalationConfig { minutes, person2 }` |
| SMS/voice credentials | SecretAnt | `SecretRef("sms-provider")` |
| Send SMS, place call | NotificAnt/CommunicAnt | `AlertChannel::sms`, `::voice` (requirement on the Ant) |
| Provider HTTP | HTTPAnt | via the channel |
| Timeout timer | SchedulAnt | one-shot job per alert, cancelled on ack |
| State, audit | PersistAnt, AuditAnt | `AlertStore`, events `alert.escalated`, `alert.acked`, `alert.unresolved` |
| Phone numbers out of logs | RedactAnt | redact numbers |

## Open questions
- SMS and voice provider (cost per message is a few cents): needs CireSnave's choice, via the PM. Recommend a provider with a plain REST API.
- Person 2's phone number is personal data: where it may be stored (SecretAnt vs ConfigurAnt).

## Channel decision (PM 2026-10-29)
A5 defines its own thin `AlertChannel` trait (send SMS, place call, report outcome); the state machine and its tests use a fake channel. No provider code in A5. SMSKit (ciresnave/smskit 0.3.0) or a direct provider becomes one adapter behind the trait later; the provider pick is parked until CireSnave decides SMSKit's future.

## Requirements for an SMS/voice adapter
Found by reading smskit@b69e34e (2026-03-30); an adapter, SMSKit or other, must supply:
- **Voice:** place a call to a number (smskit has no voice path: 0 hits for voice/call/twiml in code).
- **Retryable errors:** an error type that says whether a retry can help (smskit's `SmsError` has no such flag and its `FallbackClient` stringifies errors and moves on).
- **Redacted credentials:** secrets held via SecretAnt, never printed by `Debug` (smskit clients hold plain `pub String` tokens under `#[derive(Debug)]`).
- **Delivery receipts:** a delivered/failed outcome for a sent message (smskit handles inbound replies only).
- **Phone numbers redacted** in errors and raw provider payloads (RedactAnt).
- **HTTP via HTTPAnt**, not an adapter-private client; current dependency versions.

## Question 2026-11-10: why is person 2 not pushed through the app first?
CireSnave asked, verbatim (via the PM): *"didn't we decide that AlarmAnt was going to sound its alarm through an Android app?  If so, why wouldn't a backup person also be alerted through that same Android app running on their phone?"*

**No reason was recorded.** The SMS-then-call wording comes from the charter row written 2026-10-26 (ROADMAP A5: "tries a second person by text or call (a few cents each)"), and this design (2026-10-29) inherited it. DESIGN-A2, PATENT-SWEEP and PATENT-A5-READING give no reason for leaving person 2's app out. The only nearby rule is constraint 10 (one device per recipient, from CA 2838076), which is about one recipient's devices, not a second person's. The reason below for keeping SMS and a call is the lane's inference now, not something decided then.

**Independence, the case for SMS and a call.** An app push to person 2 fails the same ways person 1's does: Google's push service down, the app killed or never installed, Do Not Disturb or battery rules, no data. SMS and a voice call travel over separate networks. The daily self-test (A6) exists because the push path can fail silently.

**Tier order if the app is added** (each step on the same fixed N-minute timer): person 1 app, then person 2 app, then person 2 SMS, then person 2 call, then `unresolved`. The state machine gains one `Stage`; the persisted record is unchanged in shape. Code cost: the registry must hold two recipients, each with an own ack token, and an acknowledgement from either ends the alert. Person 2's app needs the same Acknowledge-only behaviour as person 1's: no "received", no "viewed", no decline.

**Patent-reading cost (a model reading, not legal advice, no clearance).**
- Granted Masimo claims (incl. US12646617B2 claims 1 and 10, which is 19/020,852): each still needs received, viewed-not-responded and declined indications from the device. An app push to person 2 adds none, so the reading stays outside as read **provided the app never reports them**. A second app instance makes it easier to add telemetry by accident; keep it out.
- It does move two elements closer in form: "sending the alarm notification to a second ... device" (published claim 1 of US20150106121A1, and "a second clinician device" in 11488711 and 11699526) becomes literal for a push, where an SMS or call was only the same function.
- US 19/662,313 (continuation of 19/020,852, filed 2026-04-29, pending) is not read. Its claims may be broader than the granted ones (claim 10 of US12646617B2 already dropped the patient-monitor wording). Until read, an app push to a second device carries more exposure than SMS and a call.
- CA 2838076 (BlackBerry, read verbatim 2026-10-31): claim 1 needs devices "associated with the same account on the server", a device-side primary test, and a delay timer on the non-primary device. Person 2's phone as its own recipient is outside as read; registering both phones under one sender account is the thing to avoid arguing, and escalation stays server-side, never between devices.

**Board 389 if the app is tier 1 for person 2.** With the app as person 2's only channel, 389 has no content: no provider, no carrier registration, no account or payment. With the app first and SMS/call kept, 389 stays but stops blocking: the app tiers ship now, the SMS/call tiers wait on the ruling behind the same `AlertChannel` trait.

## Ruling 389 (CireSnave 2026-11-12, via the PM): three channels, no SMS or voice for now
CireSnave, verbatim: *"Go with Android app alarm to first contact, Android app alarm to second contact, and alarm on my PC if neither Android app alarm is acknowledged.  That should be sufficient until we have money for SMS or voice."* This supersedes the SMS-then-call tiers above. No provider, no carrier registration, no account or payment; an SMS/voice adapter stays a later option behind the `AlertChannel` trait (SMSKit speaks Twilio and Plivo).

**Tiers, one fixed N-minute timer between each (default 5):** (1) app alarm to the first contact; (2) app alarm to the second contact; (3) PC alarm; then `unresolved` (audit `alert.unresolved`). An Acknowledge from either phone or from the PC ends the alert at any point; a phone keeps alarming until acknowledged and escalation never silences it. No reordering, no third person, no groups.

**Build order (PM):** (1) second app push on the timer, two recipients each with an own ack token, either one ends the alert; (2) patent read of the PC alarm before building it: `PATENT-PC-ALARM-READING.md`; (3) the PC alarm.

**The PC alarm:** a Windows program in the operator's logged-in user session (not a service). When the escalator reaches tier 3 it unmutes the default output endpoint, sets master volume to maximum through Core Audio and sounds a siren until the operator stops it, which sends Acknowledge. It learns of the alert by asking the server (the VPS cannot reach into a home PC); the server answers from the alert's state alone and never records that the PC asked (PATENT constraint 17). It sends no negative status (constraint 18).

**Limits to state to the operator:** a PC that is asleep, off, locked out of audio or without an output device is silent, and the server cannot know it (the alert then goes `unresolved` on its timer); the siren plays on the **default output device** only (headphones, a muted monitor speaker or a Bluetooth device may be the default); the PC must reach the sender's server by the same network path as everything else, so an outage that stops the phones' acknowledgements can also stop the PC; a user who is away from the desk hears nothing. The PC alarm is a third chance, not a guarantee.

**Registry change:** `Device` was one phone ("one device per recipient"); the registry now holds two recipients, `first` and `second`, each with a push token and an ack token. `alarmant enroll --contact first|second` makes the enrolment code for a slot. Revocation clears one slot and the in-flight ack digests of that slot (rules in `DESIGN-REGISTRATION.md` section 4 unchanged).
