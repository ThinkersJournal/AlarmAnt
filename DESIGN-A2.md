# A2 design: sender and acknowledge endpoint

Status: BUILDING (PM ruling 2026-10-31, option 2): a Rust service on the VPS. Real Ants where they exist (HTTPAnt `httpant-client` for the FCM HTTP v1 call, ConfigurAnt, PersistAnt); NotificAnt, APIAnt and SecretAnt are 0.0.0 placeholders, so they stay behind traits (`PushTransport`, `SecretSource`, `AlertStore`) with a small direct implementation, swapped in as they ship. The earlier "no sender code" hold (ant-first notice 2026-10-28) is lifted.
Constraints: PATENT-SWEEP.md design constraints apply (one alert priority, per-app override, flat escalation).

## Flow
1. A caller raises an alert (A9 API, later). 2. The sender builds one high-priority FCM data message and posts it. 3. The phone alarms until Acknowledge (A3). 4. The app calls the acknowledge endpoint; state moves `sent` to `acked` (A4). 5. No ack by the timeout: A5 escalates (blocked on A1).

## Shared Ants needed (each a stub, to be confirmed against the Ant's real API)
| Need | Ant | Stub in AlarmAnt |
|---|---|---|
| Settings (FCM project, timeouts) | ConfigurAnt | `AlarmConfig` struct, loaded by the Ant |
| FCM service-account key | SecretAnt | `SecretRef("fcm-service-account")`, never in repo |
| Outbound FCM HTTP v1 call, retry | HTTPAnt | `PushTransport` trait |
| Alert delivery | NotificAnt/CommunicAnt | `AlertChannel` trait, FCM channel implemented by the Ant if it can |
| Ack endpoint | APIAnt (+ WebHostAnt) | `POST /ack/{alert_id}` handler spec only |
| Alert state | PersistAnt | `AlertStore` trait: id, status, sent_at, acked_at |
| Ack trail | AuditAnt | event `alert.sent`, `alert.acked` |
| Log hygiene | RedactAnt | alert text redacted in logs |
| Ack timeout | SchedulAnt | one-shot job per alert |
| Daemon supervision (VPS only) | SupervisorAnt | none |

AlarmAnt's own code is only the alert model, the FCM payload shape, and the state machine.

## Ant availability (PM, 2026-10-28)
Real releases on crates.io: configurant 0.5.4, persistant 0.11.7, capant 0.0.7. Every other Ant named above is a 0.0.0 placeholder. So only ConfigurAnt and PersistAnt can be used now; the rest stay stubs until released.

## Requirement on NotificAnt
NotificAnt is not designed yet. AlarmAnt requires of it: send a high-priority FCM HTTP v1 data message (priority HIGH, no notification payload), report the send result, retry on transient failure. Stubbed as `AlertChannel`.

## Host: recommendation to CireSnave (not a decision)
Rust service on the VPS rather than a Cloudflare Worker, because a Worker cannot use the Rust Ants. No code.

## Service-account key (PM accepted 2026-10-31)
A file on the VPS outside any repo, mode 0600. Its path is a ConfigurAnt key (`fcm.service_account_file`, env `ALARMANT_FCM_SERVICE_ACCOUNT_FILE`); the JSON is read through the `SecretSource` trait (a file implementation now, SecretAnt later). Never inlined in an env var, never logged, never in a repo.

## Resolved / open questions
- Ack auth (A4, built in `src/ack.rs`): the phone sends `Authorization: Bearer <ack token>`; the sender compares it in constant time with the token stored on the alert record. Answers: 200 first ack and repeats, 401 bad or missing token, 404 unknown alert (401/404 are final, the phone should stop retrying; `AckWorker` still retries every non-2xx, a follow-up). Still open: how the token is issued at device registration (random 32 bytes, stored beside the FCM token), and its storage in PersistAnt.
- A 404 versus 401 tells a caller whether an alert id exists. Accepted: ids are random and only ever sent to the device.

## Raise-an-alert call (A9, built in `src/raise.rs`)
```
POST /alert
Authorization: Bearer <api key>
{"title": "DB down", "body": "prod-1 unreachable"}

202 {"alert_id": "<32 hex>"}
```
Title 1 to 100 characters, body 0 to 1000. Errors: 400 bad body, 401 bad key, 409 no phone registered, 410 phone token gone (re-register), 502 push refused, 503 push service busy (retry). Success means Google's push service accepted the message, not that the phone rang; the phone's acknowledgement is `POST /ack/{alert_id}` (A4). The API key is a ConfigurAnt secret (env only), compared in constant time. One device per recipient, so the registry returns one `Device`; registration (how the phone's FCM token and ack token reach the sender) is still open.

## Daily self-test (A6, built in `src/selftest.rs`)
Once a day at a configured UTC time the service raises a real alert (title `AlarmAnt self-test`) through the A9 path, so FCM, the phone's alarm and the person are all exercised. The person acknowledges it as any alert. A later `tick` after the deadline (default one hour, configurable) returns `NotAcked` if it is still unacknowledged, `Lost` if the store no longer has it, `Passed` if acknowledged; each verdict is returned once. A failed raise is returned too and retried every tick. The failure notifier must not be the phone (it may be the broken part): an email or SMS to the operator, chosen with the escalation channel (A5, blocked on A1). `SelfTest` is two values plus a flag; persist it with the alert store so a restart does not raise a second test or lose a pending verdict. A self-test never escalates to person 2. A fresh `SelfTest` raises on its first tick even if today's time has passed.
