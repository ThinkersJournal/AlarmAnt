# Device registration: design proposal (A9 gap)

Status: DECIDED by the PM on 2026-11-11 (section 8 records the rulings); docs only so far. The fresh patent reading is `PATENT-REGISTRATION-READING.md`. Code goes in slices, each with a known-bad test first (expired code, reused code, a revoked device's ack refused). No secret appears in this file or may appear in an example of it.

## 1. The gap

`POST /alert` finds the phone through `DeviceRegistry::device()`, which returns one `Device { push_token, ack_token }`. Nothing yet puts those two values there. The phone has its FCM token (`AlarmMessagingService.onNewToken` stores it in "config" prefs) but neither the sender's address nor an ack token (`AckWorker` reads `ack_base_url` and `device_token` from the same prefs and retries forever when they are missing). Registration is the one-time exchange that closes this.

Scope: one phone per recipient (PATENT-SWEEP constraint 10), one operator (CireSnave). Nothing below adds a second device, a user account or a delivery report.

## 2. Flow (proposal)

1. **Enrol (operator, on the VPS).** `alarmant enroll` creates an enrollment code: 128 random bits, shown once, valid 10 minutes, single use. It prints the sender's base URL and the code as text and as a link `alarmant://enroll?u=<base url>&c=<code>` (a QR of the link is a later nicety). Only a SHA-256 digest of the code is stored.
2. **Register (phone).** The operator pastes the link into the app, or types the two values. The app calls `POST /register` with `Authorization: Bearer <enrollment code>` and JSON `{"fcm_token": "..."}`.
3. **Sender.** Checks the code (constant-time, unexpired, unused), marks it used in the same atomic step, creates a fresh ack token (256 random bits), stores the device record (section 5), and answers `201 {"ack_token": "..."}`. This is the only time the ack token is sent; it travels over TLS only.
4. **Phone.** Saves `ack_base_url` and `device_token` (the ack token) in app-private storage, and the Setup screen turns "Registered with sender" OK.

Registering again with a new valid code **replaces** the device (a new phone). Without a fresh code nobody can replace or add one.

## 3. Who may register

Only the holder of a live enrollment code, which only the operator with shell access to the VPS can create. The API key for `POST /alert` is a different credential with a different purpose and **never** registers a device (a raiser of alerts must not be able to point alerts at its own phone). Failed `/register` attempts are rate-limited globally (proposal: 5 a minute, then 429) and answer one fixed 401 whatever was wrong (unknown, used, expired), so the answer teaches nothing.

## 4. Rotation and revocation

- **FCM token rotation** (Firebase issues a new one; `onNewToken`): `PUT /device/token` with `Authorization: Bearer <ack token>` and `{"fcm_token": "..."}`. Authenticated by the existing ack token, so no operator step. Until it lands, pushes to the old token fail with `UNREGISTERED`: `raise` answers 410 (already built), the record is marked `stale`, and the phone's retry of `PUT /device/token` repairs it.
- **Ack token rotation:** by re-enrolment only (section 2). No separate endpoint; a lost or suspect token is handled by revocation plus a new enrolment.
- **Revocation:** `alarmant revoke` deletes the device record. **It also invalidates the ack tokens of alerts still in flight** (their records stop accepting an ack from the old token), so a lost phone cannot stand down an alert and stop escalation; those alerts then escalate on their normal timer (A5). `POST /alert` answers 409 until a new enrolment. **Constraints from the patent reading (PM 2026-11-11): server-side record delete only; no re-validation or restore step for the old device; the phone never invalidates itself; no transaction roll-back.**
- **Nothing reports "received" or "viewed".** Registration and rotation record that the phone called the sender, never that an alert was delivered or seen (PM rule; `last_seen_at` changes only on register, token update and acknowledge).

## 4a. Failure when registration is missing

- `POST /alert`: 409 `NoDevice` (built). The caller knows at once, which beats a silent alert.
- Daily self-test (A6): its raise fails the same way and goes to the failure notifier, which is not the phone (A6 design), so the operator hears about it by the channel board 389 chooses.
- Phone: with no `ack_base_url` / `device_token`, `AckWorker` today retries forever. Proposal: the Setup screen shows MISSING "Registered with sender", and an ack that cannot be sent for lack of registration is kept and retried only after registration exists (still never dropped: the alarm must stop locally whatever the network or registration state).

## 5. What is stored where (PersistAnt; `DeviceRegistry` and `AlertStore` backends)

| Record | Fields | Notes |
|---|---|---|
| `device` (one row) | `fcm_token`, `ack_token_digest`, `state` (`active` / `stale`), `registered_at`, `last_seen_at` | FCM token needed in clear to send; the ack token is kept only as a SHA-256 digest (it is 256 random bits, so a plain digest is enough) |
| `enrollment` | `code_digest`, `expires_at`, `used` | deleted when used or expired |
| `alert` (A4, exists) | id, state, times, `ack_token_digest` copied from the device at send | the copy is what in-flight alerts check; revocation clears it |

Today `AckToken` keeps the token itself in memory and compares it in constant time; switching the stored form to a digest changes `AckToken::matches` to hash the presented value first. Small, and listed as a follow-up code change.

On the phone: app-private preferences only; both values are excluded from backup (the existing backup rules already exclude everything).

## 6. What is never logged

The FCM token, the ack token, the enrollment code, the API key, the service-account key, and any `Authorization` header or `/register` body. `Debug` of every type holding one prints `<redacted>` (as `AckToken` and `DeviceToken` already do). Audit events carry names only: `device.registered`, `device.token_updated`, `device.revoked`, `device.stale`; with timestamps and no values. Error answers use fixed text and never echo input (tested the same way as the API key in #45).

## 7. Patent note

Registration is plumbing: no alert priority, ordering, escalation or delivery report is added, which keeps to the PATENT-SWEEP design constraints (one priority, flat escalation, no phone-side "received"). A fresh reading for this change is in `PATENT-REGISTRATION-READING.md` (thin: three searches, three patents read; it adds five constraints, including that `/register` returns no push-service certificate). The sweep stays incomplete: "outside as read", never "clear".

## 8. Rulings (PM, 2026-11-11)

1. Enrolment: operator-created code, single use, 10-minute expiry, 128 random bits, constant-time compare, never logged.
2. Ack token stored as a digest only.
3. FCM token in clear in PersistAnt with file permissions for now; revisit when SecretAnt ships; never logged.
4. Revocation kills in-flight acks: yes.
5. `/register` rate limit 5 a minute, **counted on failed attempts only**, so a flood of bad codes cannot lock out a real enrolment; a valid code always passes (the limiter must not run before the code check in a way that rejects a valid code).
6. Operator CLI `enroll` / `revoke`; no admin HTTP routes.

Slices (each starts with its known-bad test): (a) `AckToken` stores a digest; (b) enrolment store and code check (expired, reused, wrong); (c) `POST /register` with the failed-attempt limiter; (d) `PUT /device/token`; (e) revoke, including invalidating in-flight acks; (f) CLI; (g) PersistAnt backends; (h) Android registration step.
