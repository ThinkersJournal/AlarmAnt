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
