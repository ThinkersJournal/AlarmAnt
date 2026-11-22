# Changelog

## 0.4.0 — **BREAKING**: `POST /alert` answers 429 above 6 alerts an hour or 20 a day per key (settable)

ROADMAP A10 (opened 2026-11-20 by pm-ants): the endpoint had no rate or count limit, so any holder of the API key could raise alerts without bound. A new `ratelimit` module counts raised alerts per key **owner** over a rolling hour and a rolling day (defaults 6 and 20; `ALARMANT_RATE_PER_HOUR` and `ALARMANT_RATE_PER_DAY` change them, a non-number or 0 refuses to start). The check runs after authentication: a wrong key is the usual 401 and spends nothing; an over-limit alert answers `429 {"error":"too many alerts; try again later","owner":"api","dropped":N}`, sends **no push of its own**, is not counted (being refused does not extend the wait) and is noted on stderr (owner and count, no text). **A refusal is never silent (pm-ants review):** the first refusal in an hour pages the human once, "Alerts throttled for key owner api", saying later alerts are dropped until `HH:MM UTC` (when the budget has room again); that notice is exempt from the cap and is itself limited to one an hour per owner, and later refusals only raise the `dropped` count in the answer and on stderr. The count is not in the page (it is unknown when the page goes out); it is in every refused answer and in the log. Owners, not keys: there is one API key today, so one owner; keys of one sender must name the same owner when more keys exist, so a second key cannot double the budget (tested at the limiter). Counts are in memory, so a restart forgets them. Red first: the new serve test failed with 202 where 429 was expected against a limiter stub that allowed everything; sabotages: `>` for `>=` fails five tests, a notice on every refusal fails two. 208 Rust tests pass, fmt, clippy and `cargo deny` clean. A sender that raises more than 6 an hour will now be refused: breaking for such a caller, hence the major bump pre-1.0. Not run: a live server.

Also new: `GET /alert/:id` (API key) answers `{"state":"open"}` or `{"state":"acked"}` and nothing else (401 wrong key, 404 unknown id): groundwork for the Synapse bridge (ruling 478).

Also new: `tests/constraints.rs` (ruling 407-A) fails if the phone sends anything but Acknowledge, the PC program adds an outgoing call, a public route is added, or the escalator gains an input beyond a raise, an Acknowledge and the timer; ROADMAP row A13 records the dated re-read of 19/662,313.

Also fixed: pm-ants sweep (2026-11-20): our copy was not text-identical to the canonical Apache-2.0 (words missing in sections 2 and 3, "authorship covered by this License" for "authorship"). Replaced the whole file with ciresnave/PersistAnt `LICENSE-APACHE` (11358 B); the committed blob's sha256 is cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30, read back from the index. Licence text only; no code change.

## 0.3.1 — Docs only: design note for the human delivery adapter and the independent watchdog (replaces the earlier Synapse notes)

`docs/SYNAPSE-ADAPTER.md` (PM task 2026-11-20 from `SYNAPSE-MESSAGING-BRIEF.md`): a no-socket bridge on the Synapse machine raises the existing alert for urgency-high mail to a human account and returns the acknowledgement as Synapse mail; the draft mail format and envelope fields to agree through pm-ants; no phone nack (PATENT-SWEEP constraint 13), flagged for CireSnave; per-key, per-sender and overall rate caps on `POST /alert` (none exist today); an independent watchdog of three observers (server-side heartbeat dead-man, PC-side check, optional external dead-man) with a false-alarm-storm design and a stated gap without the external one; threat models; the published `synapsed-client` (rc.123 published, rc.262 in the repo); patent table with constraint 8 read and a fresh reading owed (date set) before code. No code, nothing run, nothing live; AlarmAnt stores no alarms and has no agent path.

## 0.3.0 — **BREAKING**: the Android app accepts only `https://` enrolment links; the server can serve HTTPS; re-enrolling voids the replaced phone's ack tokens

Four changes on one branch. (1) `register()` also voids the ack token of the device it replaces, on every in-flight alert, exactly as `revoke` does (PM ruling 2026-11-19, option a); server-side only, nothing sent to the old phone, nothing restored, a first enrolment voids nothing. Test written first and seen failing; a sabotage check (voiding the new token instead) fails it. `RegisterConfig` gains `alerts`. (2) `Registration.parseLink` rejects `http://` (the enrolment code, FCM token and lifelong ack token must not cross a network in the clear; the manifest already forbids cleartext). A phone already enrolled with an `http://` base keeps that stored address and will not reach an https server: it must re-enrol with an https link. Phone side only; the Kotlin unit tests were NOT run here (no JDK): the changed `parseLink` assertions (https accepted, `http://h&c=abc` rejected) were exercised only by CI. (3) `alarmant serve` serves HTTPS when `ALARMANT_TLS_CERT_FILE` and `ALARMANT_TLS_KEY_FILE` (PEM, both or neither) are set; neither keeps plain HTTP for use behind a proxy; errors are fixed text, never key material. `httpant-server` and `httpant-client` move to 0.7.10 (server with its `tls` feature); `rcgen` 0.14.10 is a dependency. (4) A home LAN without a public certificate: `ALARMANT_TLS_SELF_SIGNED_NAMES` (IPs and host names) makes a self-signed pair once in `ALARMANT_DATA_DIR` (key created exclusively, mode 0600 on Unix), reused afterwards; `alarmant enroll` with `ALARMANT_PUBLIC_URL` prints the full link, carrying `f=<SHA-256 of the certificate>`. The phone (new `PinnedTls`, the only custom trust code in the app) accepts only a certificate whose SHA-256 equals `f=` for the enrol, token-update and ack connections, keeps the default hostname verifier, and uses the system trust store when the link has no `f=`; a malformed `f=` rejects the link. Patent reading `PATENT-TLS-PIN-READING.md` (US2018/0302787, US9930028, US11451535, US2020/0396610: all outside as read). Review fixes (code-review high): the self-signed pair is written certificate-last through temporary files and renamed (an interrupted run is repaired on the next start), a pair made for other names is refused with the files to delete instead of being served with a stale name, `alarmant enroll` pins only when `ALARMANT_TLS_SELF_SIGNED_NAMES` is set and requires an `https://` `ALARMANT_PUBLIC_URL`, finds the code by its shape, not by line position, the PC alarm trusts the self-signed certificate through `ALARMANT_PC_SERVER_CERT_FILE` (tested over a real TLS handshake, with a control that refuses an unconfigured client and another certificate), registration serialises and stores the new device, then voids the old token (the order `revoke` uses), an `http://` enrolment stored by an older app now reads as unregistered, and the app no longer crashes on an unusable pin, refuses a pin on a non-https URL instead of ignoring it, and treats a pin mismatch as permanent (the ack worker stops and registration reports a rejection) instead of retrying for ever; `alarmant enroll` also refuses a host the certificate was not made for and an unset `ALARMANT_DATA_DIR`, and names are compared case-insensitively. On Windows the key file is not made owner-only (the data directory must be private). Verified: 200 Rust tests pass (new: fingerprint, self-signed made once and reused with an IP SAN, half-a-pair and no-name refused, link encoding, plus the TLS tests above), clippy clean, `cargo deny` ok. Kotlin unit tests were NOT run here (no JDK); only CI exercises `PinnedTlsTest` (right fingerprint accepted, wrong fingerprint refused, another valid certificate's fingerprint refused, bad pin forms refused, no pin keeps the default socket factory, a pin gets its own factory and leaves the default untouched) and the new `RegistrationTest` pin assertions. Not run: a live TLS handshake from the phone, a phone, Firebase.

## 0.2.11 — Escalation state and the PC-alarm board survive a restart

The served app used an in-memory escalator, so a restart dropped every open alert's timer and the PC-alarm board. `PersistedEscalations` (`escalations.json`, PersistAnt atomic replace, a corrupt file refused not overwritten) now backs the escalator, and `App::with_escalations` resumes the open alerts and puts every alert already at the PC-alarm stage back on the board; `alarmant serve` uses it. A timer that came due while down fires one step on the next tick; the second app push is not repeated. Tests: records round-trip and an ack removes one, a corrupt file is refused, and a restart restores the board (checked failing with the re-seed removed). Rust tests: 183 pass locally; nothing run live.

## 0.2.10 — Android release signing and release workflow

`assembleRelease` signs from `ALARMANT_KEYSTORE_*` environment variables when set (unsigned otherwise), and `.github/workflows/android-release.yml` builds and verifies a signed APK only on a pushed `android-v*` tag, keeping it as a workflow artifact. `ANDROID-RELEASE.md` lists the secrets and steps. Unverified: nothing has been run with a real keystore; the PR build proves only that the unsigned build still works.

## 0.2.9 — Android pushes a rotated FCM token to the sender

When Firebase rotates the push token and the phone is registered, a WorkManager job calls `PUT /device/token` with the ack token as bearer (registration slice h, part 2; the server route already existed). Answer mapping and the registered check are unit-tested (`TokenUpdateTest`). A 401 stops quietly and changes nothing on the phone (no self-invalidation); nothing is reported beyond the phone's own call. Unbuilt locally (no JDK): CI is the check. Stacked on #68.

## 0.2.8 — changed

HANDOFF.md rewritten to the current state: server chain, admin endpoint, binary and PC alarm are on main; what is in flight (#66, the A8 signing WIP branch), what is next (Android registration step), and the known gaps. Documentation only.

## 0.2.7 — Android registration step

The Setup screen shows "Registered with sender" and, until it is OK, a field for the enrolment link (`alarmant://enroll?u=..&c=..`) and a Register button that calls `POST /register` with the code as bearer and stores the base URL and ack token (registration slice h). Link parsing, the registered check and the answer handling are unit-tested (`RegistrationTest`); secrets are never logged and failures show fixed text. The phone reports nothing but its own register call (PATENT-SWEEP constraint 13). Not yet built or run: no JDK on this box, so CI is the check. Not in this slice: `PUT /device/token` on FCM rotation.

The README status line is rewritten to match what is built and what is unverified (claim evidence for the portfolio PM, 2026-11-16).

## 0.2.6 — changed

CI: the shared portfolio workflow moves to coderipper c72a6e03 (CodeRipper 0.47.5). No rule names to migrate: the repository has no `.coderipper.toml` and names no rule in any file (the migration tool exits 2 on the missing config; a grep for the 14 old names finds none). Verified by reading the Portfolio job of this PR.

## 0.2.5 — added

The PC alarm program, `alarmant-pc` (Windows, in the operator's logged-in session): it polls `GET /pc/alarm` every 5 s; while an alert is at the PC step it unmutes the default speakers, sets master volume to maximum through Core Audio (re-asserted on each poll), loops a two-tone siren, and shows a topmost Stop dialog whose OK acknowledges the alert (`POST /pc/ack/:id`, PC key, accepted only for an alert at the PC step; it ends the alert for everyone). The portable logic is `pcclient` (`PcClient`, `Watcher`, the `Siren` trait). It only asks and only ever sends that Acknowledge: no status of any kind goes to the server, a failed poll changes nothing (a sounding siren keeps sounding), the key goes only to https or this machine, and `Debug` never shows it (constraints 13, 17 to 22). Verified by tests of the watcher, the key guard, polling and refusal against the real route, the server ack route (refused before the PC step, 401 changes nothing, 200 ends the alert) and the siren WAV. Not run live: starting it changes the machine's volume and plays a siren. Limits: needs the PC on, the user logged in, the program running and a playback device; cannot wake a sleeping PC; headphones or a muted external amp still make it inaudible.

## 0.2.4 — added

The `alarmant` binary. `alarmant serve` runs the public routes with the escalation timer wired in (`serve::public_server`: `POST /alert` starts the timer, `POST /ack/:id` ends it, plus `/register`, `PUT /device/token`, `GET /pc/alarm`), the loopback admin endpoint, persisted stores, and a 5 s timer loop (`serve::tick_once`: fire due steps, then push the second contact). `alarmant enroll|revoke --contact first|second` is the thin client to the admin endpoint. Secrets come from the environment or a key file, never argv; messages never repeat one. `EscalationStore` is now `Send` so the escalator can sit behind the routes. Verified by two end-to-end tests over real HTTP (first push, second app at N, PC alarm at 2N, either ack ends it; an acked alert never reaches the second contact) and by running the CLI's error paths. Not run live: it needs a Firebase service account (board 354).

## 0.2.3 — added

The server's `AlertChannel` (`channel::Dispatcher`): the second contact's app push is queued and sent by `drain_second` (via `push_second`), and the PC alarm is raised on a `PcAlarmBoard` that the PC program polls at `GET /pc/alarm` with its own bearer key (constant-time compare, fixed 401, never cached). An acknowledged alert drops off the board. In-memory only: an alert already at the PC-alarm stage is not raised again after a restart. Verified by four tests; removing the key check fails the route test.

## 0.2.2 — added

Registration (g), part 2: the loopback admin endpoint and its client (PM ruling 2026-11-13). `adminapi::admin_routes` adds `POST /admin/enroll` and `POST /admin/revoke` (`{"contact":"first"|"second"}`) for a separate server bound with `bind_loopback`, which refuses any non-loopback address. The admin key (`admin_key_from_env`, `admin_key_from_file`; digest only, never argv, logs or error text) is checked first in constant time, before the body is read, so a wrong or missing key answers a fixed 401 and mutates nothing; commands run one at a time under one lock. `adminapi::send` is the client for `alarmant enroll|revoke`: it refuses a non-loopback base URL so the key cannot leave the machine, and reports failures in fixed text. `persist::Persisted*` now hold an `Arc<Disk>` so the stores can sit behind the routes. The binary that wires this follows. 167 tests; removing the key check fails two named tests, removing the loopback guard fails one.

## 0.2.1 — added

Registration (g), part 1: the stores on disk through PersistAnt (`persistant` 0.17). `persist::Disk` opens a data directory (one JSON file per store, replaced atomically); `PersistedDevices`, `PersistedAlerts` and `PersistedEnrollments` are the in-memory stores with write-through snapshots, loaded once at open, so devices, alerts (with both ack digests and the alert text) and pending codes survive a restart, and a revocation stays a revocation. On disk, ack tokens and enrolment codes are SHA-256 digests only; the FCM push token is in clear (needed to send), so the data directory must be readable by the service account only until SecretAnt. A corrupt file is refused and left untouched; a write that fails stops the service with fixed text rather than let a restart undo a change. The server binary and the loopback admin endpoint for `enroll`/`revoke` (PM ruling 2026-11-13, in ROADMAP.md) follow. 161 tests; skipping the save in `take_for` fails `a_revocation_is_still_a_revocation_after_a_restart`.

## 0.2.0 — **BREAKING**: `EnrollmentStore::consume` now answers the contact; `DeviceStore` is keyed by contact

Three-channel step 1c (board 389): the second contact can be enrolled, registered and revoked. `enroll::Contact` (First, Second) replaces `admin::Contact` (still re-exported); an `Enrollment` names its contact, the store holds one pending code per contact (a new code for one leaves the other's alone), and `EnrollmentStore::consume` returns the contact the code was for. `DeviceStore` gains `set_for`/`take_for` (`set`/`take` are the first contact's), `MemoryDevices` holds both slots, `POST /register` stores the device in the slot its code was made for, and `PUT /device/token` rotates the push token for whichever contact's ack token is presented. `revoke::revoke_contact` and `alarmant enroll|revoke --contact second` now run (the refusal and `AdminError::Unavailable` are gone); revoking one contact deletes only that device and voids only its tokens. Nothing new is reported to a phone. 154 tests; limiting the token-rotation check to the first slot fails a named test.

## 0.1.1 — added

Three-channel step 1b (board 389): the second contact's app push. `second::push_second` sends the same alert text to the second recipient (`DeviceRegistry::second_device`, default none) and records their ack token on the alert (`AlertRecord::second_ack_token`, `AlertStore::set_second_ack_token`); `acknowledge` then accepts either contact's token and ends the alert. Revoking a device voids its token on in-flight alerts whichever slot it holds, a push racing a revocation of the second device leaves no working token, and an acknowledged alert is never pushed again. Alert text is now kept on the record (`AlertText`) so a later tier can push it. The phone still sends only an Acknowledge. Registering and revoking the second contact (CLI and routes) follow. 150 tests; dropping either-token acceptance, or the race recheck, fails a named test.

## 0.1.0 — **BREAKING**: escalation tiers are now second contact's app, then PC alarm

Three-channel step 1a (board 389). The escalation state machine no longer texts or calls a phone number: on the fixed N-minute timer it now goes first-contact push, then `AlertChannel::second_app`, then `AlertChannel::pc_alarm`, then unresolved; an acknowledgement from anyone still ends it from any state. Removed: `Contact` (phone number), `AlertChannel::sms`/`voice`, `Stage::Sms`/`Voice` (now `SecondApp`/`PcAlarm`), `EscalationRecord::person2`, the `person2` argument of `Escalator::register`, `Alert::new`/`restore`, `EscalationConfig::new`/`with_default_minutes`, `ConfigError::Number`, and the `config` argument of `raise_escalating`. The phone still sends nothing but an acknowledgement and the timer fixes the pace (constraint 13). Adapters for the second push and the PC alarm, and the second recipient in the registry, follow in later slices. 140 tests, clippy clean.

## 0.0.60 — added

Registration (f): operator commands. `alarmant enroll --contact first|second` makes the one-time enrolment code and shows it once; `alarmant revoke --contact first|second` deletes the device and voids its in-flight ack tokens. `--contact` is required and never guessed; there is no command that undoes a revocation. This slice is the command logic (`admin::parse`, `admin::run`); the binary and PersistAnt stores come with slice (g). Only `first` runs until the registry holds two recipients; `second` is refused with a plain message and changes nothing.

## 0.0.61 — docs: HANDOFF rewrite

- HANDOFF rewritten for the current state: board 389 ruled, registration stack, three-channel build order, A8 signing draft, phone-session readiness.

## 0.0.59 — registration slice (e): revoke

- Registration slice (e): `revoke` module. `revoke(devices, alerts)` deletes the device record (`DeviceStore::take`) and voids the ack token of every alert still in flight (`AlertStore::void_ack_tokens`, token becomes `AckToken::void`, an all-zero digest nothing can match), so a lost phone cannot stand down an alert; those alerts escalate on their normal timer. Acknowledged alerts, other devices' alerts and a second revoke are untouched or harmless; afterwards `POST /alert` answers 409 (no device) and the old token cannot rotate a push token. A raise racing a revocation rechecks the registry after storing its record and voids it too. Patent rules kept: server-side record delete only, no restore or re-validation step (none exists to call), the phone is told nothing and never invalidates itself, no roll-back. Known-bad stub (does nothing) failed 5 of 6 tests first; the race test failed before the recheck; mutation check: dropping the acknowledged-alert guard fails the acknowledged test. 7 new tests. Left for later: operator CLI (f), PersistAnt (g), re-enrolment does not void the replaced phone's in-flight alerts (revoke first), audit events.

## 0.0.57 — registration slice (d): PUT /device/token

- Registration slice (d): `update_token` and `PUT /device/token`. The phone presents its ack token as `Authorization: Bearer` with `{"fcm_token"}` after Firebase rotates its token; the push token is replaced in one step with the credential check (`DeviceStore::replace_push_token`), the ack token is kept. The credential is checked before the body, so a stranger gets 401 whatever they send; missing, wrong, enrolment-code-as-bearer, digest-as-bearer and no-device all answer one fixed 401; a bad body with the right token is 400 and changes nothing; 413 over 16 KiB; answers are `no-store`. Nothing is reported about delivery and nothing escalates (patent rule: the phone sends no status the sender acts on). Known-bad stub (accepts anything) failed all 4 new unit tests first; mutation check: matching any non-empty token fails the 401 test. 5 new tests (4 unit, 1 over real HTTP). Left for later slices: revocation, CLI, PersistAnt, stale marking on UNREGISTERED.

## 0.0.56 — registration slice (c): POST /register with a failures-only limiter

- Registration slice (c): `register` module and `POST /register` route. Bearer enrolment code plus `{"fcm_token"}`; a valid code makes the phone the device and answers `201 {"ack_token"}` once (`Cache-Control: no-store`; only the digest is kept); wrong, missing, reused and expired codes answer one fixed 401. `FailureLimiter` counts failed attempts only (5 in 60 s, then 429): the code is always checked first, so a valid code passes after any flood. A bad body is 400 before the code is spent; 413 over 16 KiB. `DeviceStore` and `MemoryDevices`. Known-bad stub (accepts anything) failed the 401 and 429 tests first; mutation check: not recording failures fails the 429 test. 10 new tests (8 unit, 2 over real HTTP). Left for later slices: audit events, PersistAnt backends, revocation.

## 0.0.55 — registration slice (b): one-time enrolment code and store

- Registration slice (b): `enroll` module. `new_enrollment` makes a 128-bit code (32 hex digits, shown once) and its `Enrollment` (SHA-256 digest via `AckToken`, 10-minute expiry); `EnrollmentStore` with an atomic `consume` (right and unexpired: accepted once and removed; right but expired: removed and refused; wrong or reused: refused, and a wrong guess does not burn the real code); one pending enrolment, a new one replaces the old. `MemoryEnrollments`. Known-bad tests first (a stub that accepted anything failed 6 of 9: expired, reused, wrong, replaced, none pending, race); mutation check: `>=` to `>` at expiry fails the expiry test. No HTTP route yet (slice c).

## 0.0.54 — docs: A5 three-channel ruling and PC-alarm patent reading

- Board 389 ruled (CireSnave 2026-11-12): app alarm to the first contact, app alarm to the second contact, then an alarm on the operator PC; SMS and voice wait. `DESIGN-A5.md` gains the ruling, tiers, build order, registry change and the PC alarm limits. `PATENT-PC-ALARM-READING.md` reads the design against US 2026/0360532 A1 claims 19 to 38 (claim 27 included) and four desktop audio-override patents read verbatim: outside as read, no claim element present; constraints 17 to 22. Not legal advice; no clearance.

## 0.0.52 — docs: patent claim map for US 2026/0360532 A1

- Patent sweep: added `PATENT-MASIMO-19-662313.md`, an element-by-element map of claims 19 to 38 of the Masimo continuation (appl. 19/662,313) against AlarmAnt. Outside as read: the phone sends no "not accepted or unavailable" reply and escalation is a fixed timer, not a response to a reply. Adds design constraints 13 to 16 to `PATENT-SWEEP.md`. Not legal advice; no clearance.

## 0.0.49 — tooling: changelog fragments and version stamp

- Adopt the portfolio changelog-fragment workflow: `changelog.d/` fragments, `tools/stamp-version.py` (run by the gate at merge), `tools/check-fragment.py` and the `changelog-fragment` CI check. PRs now add one fragment and never edit CHANGELOG.md, Cargo.toml or Cargo.lock versions.

## 0.0.48
- Registration slice (a): `AckToken` keeps only a SHA-256 digest of the token (`ring`), never the token. `matches` hashes the presented value and compares the digests in constant time; new `digest_hex` (what to persist) and `from_digest_hex`. Known-bad test first: `a_leaked_digest_cannot_ack` failed against the plaintext implementation, passes now; a second test covers the round trip and malformed digests. The A9 API key uses the same type, so it is digest-only too. Debug/Display still redact. Behaviour for callers is unchanged. PM-approved 2026-11-11; version number is a placeholder.

## 0.0.47
- Docs only: `DESIGN-REGISTRATION.md`, the device-registration proposal (A9 gap): enrolment code, `/register`, FCM token rotation, revocation that also invalidates in-flight acks, what is stored in PersistAnt, what is never logged, the failure when registration is missing, and six open questions for the PM. Rulings recorded (PM 2026-11-11) and `PATENT-REGISTRATION-READING.md` added (a model reading, not legal advice). No code yet.

## 0.0.46
- A7: `android/SIDELOAD.md`: build the debug APK, install with `adb install -r` or from the Files app, the first-run grants, updating, and what is not yet possible (registration, real pushes, release signing, physical-device testing). Docs only; nothing executed on a phone.

## 0.0.45
- A9: the HTTP route and key config (key compared in constant time via `AckToken::matches`, never logged; a test that no answer contains it). `routes::raise_route` adds `POST /alert` (Bearer API key, JSON `{"title","body"}`; 202 `{"alert_id"}`, 400, 401, 409, 410, 413 body over 16 KiB, 502, 503; JSON answers) from a `RaiseConfig`. `api_key_from_env` / `api_key_from_value`: the key comes from an environment variable only, at least 32 characters, whitespace trimmed; errors name the rule, never the value. 3 new tests over real HTTP; mutation check: ignoring the request's Authorization header fails the 401 test. Device registration is still open (fixed config registry until PersistAnt).

## 0.0.44
- A4: the HTTP route. New `routes` module: `ack_route` adds `POST /ack/:alert_id` to an HTTPAnt `Server` (200 first and repeat ack, 401 bad or missing token, 404 unknown alert; empty body), `Clock`, `system_clock`. 3 tests over real HTTP on 127.0.0.1; mutation check: dropping the Authorization header fails the first-ack test. `AlertStore` and `DeviceRegistry` now require `Send + Sync` (a handler future holds them across an await); all in-tree implementors already were. New dependency `httpant-server` 0.7.5; `httpant-client` raised to 0.7.5 to match.

## 0.0.43
- Docs only: HANDOFF current after #42.

## 0.0.42
- Android: `AckWorker` stops (Result.failure) on 401 and 404 instead of retrying forever; 5xx, other codes and I/O errors still retry. Classifier `AckWorker.outcome` unit-tested.

## 0.0.41
- A6: `run_loop` and `Schedule`: the scheduler loop around `run_step`. Runtime-agnostic (caller gives the clock, the sleep and the stop flag); sleeps the time to the step's wake, clamped to 1 s..`MAX_SLEEP_SECS` (3600), so it never busy-loops and a far wake or a clock jump is reached in hops. No provider and no notifier channel chosen (PM 2026-11-11: trait and fake only; board 131/389 stay open). 3 tests on a simulated clock; mutation checks: dropping the sleep cap and changing the retry delay each fail a test.

## 0.0.40
- Docs only: HANDOFF brought current (A2 2b and the setter hardening are on main; only the board 389 wait remains).

## 0.0.39
- Harden FCM base URL setter: `HttpPushTransport::with_fcm_base` is now test-only (`#[cfg(test)]`, `pub(crate)`), like `ServiceAccount::with_token_uri`, so the shipped transport can only use the built-in endpoint. A `compile_fail` doctest fails if it becomes public again.

## 0.0.38
- A2 slice 2b: `push` module: `HttpPushTransport`, the real `PushTransport` over HTTPAnt (`httpant-client` 0.7.1 with `https`): service-account sign-in cached by `TokenCache`, FCM HTTP v1 POST with the bearer token, answer classified (2xx accepted; 404 `UNREGISTERED` is `TokenGone`; 429, 5xx and network failures transient; a 401 clears the token and retries once; the rest permanent). Errors are fixed text or a short provider code, never a token, the key, the device token or a body. Endpoints are configurable; 13 tests run against a local fake server, no real FCM call (key is board 354). New dependencies: `httpant-client`; dev: `tokio`. `cargo deny` licences, bans, sources and advisories pass with the existing deny.toml. Mutation checks: no cache clear on 401, UNREGISTERED not mapped, token never cached each fail a test. Auth-framework rc shape not used: 2b runs on `TokenCache` as is.

## 0.0.37
- A6: rest of the provider-free part: `SelfTest::to_json`/`from_json` and `SelfTestStore` (+ in-memory), `SelfTest::next_wake` (when to tick next: now if due or ended, else the sooner of the next daily slot and the pending deadline), `run_step` (load, tick, save, notify, next wake; a failed raise retries after `retry_secs`; unreadable state starts afresh), and the `FailureNotifier` trait with `notify_failures`. The failure channel is NOT chosen (tied to A5; asked the PM). No timer loop (no runtime dependency yet). 8 new tests; mutation check: naming the new test instead of the ended one fails the verdict-plus-raise test.

## 0.0.36
- Docs only: DESIGN-A5 answers CireSnave's question on alerting person 2 through the app: no reason was recorded for SMS and a call; independence argument, app-first tier order, patent-reading cost (US 19/662,313 still unread), and what board 389 becomes.

## 0.0.35
- Docs only: HANDOFF brought current (A5 slices 1, 2, 5 and restart tests on main; only the board 389 wait remains).

## 0.0.34
- A5: six restart tests for `Escalator`: resuming sends nothing, every overdue alert fires exactly one step after a long outage, stage and due are saved after each step, restart after the call ends unresolved without a second call, a stored acked record is dropped on load, a not-yet-due timer waits. Tests only; mutation check: giving an unresolved alert a new timer fails the stage/due test.

## 0.0.33
- A5: `wire` module: `EscalationConfig` (N 1 to 120 minutes, person 2 as + and 8 to 15 digits; errors never carry the number), `raise_escalating` (raise, then start the timer) and `acknowledge_escalating` (ack, then cancel it). An ack that races the raise response starts no timer: register and cancel both run under the escalator lock after the store is read. 6 tests. Still no provider, no ConfigurAnt/PersistAnt backend, no new phone input.

## 0.0.32
- A5: escalation state survives a restart: `EscalationStore` trait (write-through), `EscalationRecord`, `MemoryEscalationStore`, `Escalator::with_store` resumes open alerts; a timer that came due while down fires one step on the next tick. `Alert::restore` and accessors, `Stage` now public. Person 2's number still never prints. 3 new tests. No PersistAnt backend, provider or phone input yet.

## 0.0.31
- A5: `escalate` module: `Escalator` runs the escalation state machine on one fixed timer (N minutes, default 5, 0 raised to 1): `register`, `ack`, `tick(now, channel)`, `next_due`. A late tick fires one step only; an ack cancels the timer; unresolved alerts have no timer and end only on ack. No received/viewed/decline input, no provider, no persistence yet. 7 unit tests with a fake channel.

## 0.0.30
- A2: `auth` module: Google service-account sign-in for the sender: `ServiceAccount::from_json`, RS256 JWT assertion (`ring`), token request body, token response parsing, `TokenCache` (refresh a minute early). Key and token never print; errors carry no secret. 10 unit tests, signature verified against the public key. New dependencies: `ring`, `base64`. No network yet: the HTTPAnt call is the next slice. Does not touch escalation.

## 0.0.29
- A1: third patent pass: Google Patents route with positive control, 3 queries, independent claims of 26 documents read verbatim with patent-claims.py (outside as read), constraints 11 and 12, Masimo family of 7 read verbatim and mapped against A5 in PATENT-A5-READING.md for a person to confirm. Docs only; the sweep stays incomplete.

## 0.0.28
- A6: `selftest` module: daily self-test through the real raise path (`SelfTest::tick`); previous test reported once as Passed, NotAcked (deadline) or Lost (sender has no record); a failed raise is returned and retried next tick. `raise_authorised` split out of `raise_alert`. 7 unit tests. No scheduler, persistence of `SelfTest` or failure notifier yet.

## 0.0.27
- A9: `raise` module: `POST /alert` handler (`raise_alert`): bearer API key, JSON `{title, body}` (title 1 to 100 chars, body up to 1000), stores the alert then pushes it, answers 202 `{alert_id}`; 400/401/409/410/502/503 mapped. `AlertStore::remove`, `getrandom` dependency for 128-bit alert ids. 9 unit tests with a fake transport. No HTTP server yet. Test fix: AlarmPathTest's last assertion ran after Acknowledge had destroyed the activity (NullPointerException from `onActivity`, failed CI once); it now waits for the DESTROYED state.

## 0.0.26
- A1: assignee queries (Atlassian, Splunk, PagerDuty) recorded in PATENT-SWEEP; they reached no patent pages, so they close nothing. Docs only.

## 0.0.25
- A4: `ack` module: framework-free `POST /ack/{alert_id}` handler (`acknowledge`), `AckToken` (constant-time compare, never prints), `AlertStore` trait with `MemoryStore`, per-alert record (state, sent_at, acked_at). First ack answers 200 once, a retry 200, wrong or missing token 401, unknown alert 404. 10 unit tests including 16 racing acks. No HTTP server or durable store yet.

## 0.0.24
- A2: `fcm` module: FCM HTTP v1 alert body (high-priority data message, no notification block, bounded TTL), `DeviceToken` (never prints), `PushTransport` and `SecretSource` traits. No network code yet. DESIGN-A2 records the PM ruling of 2026-10-31.

## 0.0.23
- A3/A8: instrumented tests for the alarm path (6 tests, pass on API 36); fix: ACTION_STOP used stopSelf(), which dropped a START queued behind it and crashed with ForegroundServiceDidNotStartInTime (now stopSelf(startId)). CI jobs `android` and `android-instrumented`.

## 0.0.22
- A3: Android lint run on main (25 warnings -> 9, 0 errors); fixes and dispositions in android/README "Lint". KTX prefs/Uri helpers, backup rules, API 26 lock-screen flags, InlinedApi suppressions with reasons.

## 0.0.21
- A3: AlarmManager watchdog. While an alarm is unacknowledged, a 5 s exact alarm restarts the service if its process died (silent gap about 30 s -> 2 to 7 s). New permission SCHEDULE_EXACT_ALARM, new Setup step.

## 0.0.20
- A3: AlarmActivity follows Do Not Disturb live (Total Silence warning appears or clears while the alarm sounds). Emulator rows: fresh-install kill, Recents swipe measured. Docs and activity only.

## 0.0.19
- A3: Total Silence warning in SetupActivity and AlarmActivity (the platform mutes the alarm there and no app can bypass it).

## 0.0.18
- A3: process-death recovery. AlarmService returns START_REDELIVER_INTENT, so the system restarts a killed alarm until Acknowledge (STOP stays NOT_STICKY).

## 0.0.17
- A3: alarms survive a reboot. ActiveAlarm persists the sounding alert until Acknowledge; BootReceiver restarts it (falls back to a full-screen notification if Android refuses the foreground start).

## 0.0.16
- A3: emulator runs for DND, process death, reboot and battery whitelist; findings in android/README. Docs only.

## 0.0.15
- A3: first run on an emulator (API 36 google_apis x86_64, WHPX): foreground alarm service, max-volume alarm-stream playback, full-screen AlarmActivity on a locked screen, back blocked, Acknowledge stops audio/service and restores volume. Docs only.

## 0.0.14
- HANDOFF rewrite (docs only).

## 0.0.13
- A3: android/README build, lint and unit-test commands and the toolchain versions used; HANDOFF rewrite. Docs only.

## 0.0.12
- Patent sweep: the three published applications (CA 2838076, US2024/0277934, US20160086473) read from Google Patents; all clear, two new design constraints (9: second notification never to a medical device; 10: one device per recipient). CA 2838076 claims are summarised, not verbatim. Docs only.

## 0.0.11
- A3: first unit tests (JUnit, no device): the alert de-duplication window (accept, reject repeat, evict oldest at 200). The window logic is now a pure function. No Rust change.

## 0.0.10
- A3: fixes from the first lint run. Foreground start now works on API 26-28 (ServiceCompat); setup walkthrough no longer reports Notifications/Full-screen as MISSING on releases that grant them at install; alert de-duplication evicts the oldest ids (StringSet was unordered); a repeat alarm start no longer leaks a wake lock; alarm tone falls back to ringtone then notification tone instead of staying silent. `lintDebug` passes with no errors. No Rust change.

## 0.0.9
- A3: first build of android/ (debug APK assembles with JDK 17, Gradle 9.8.2, AGP 9.5.0, compileSdk 37). Added the Gradle wrapper (checksum-pinned); compileSdk 36 -> 37 as the dependencies require. Still unrun on a device or emulator. No Rust change.

## 0.0.8
- A3: Android project skeleton under android/ (service, alarm playback, full-screen activity, ack worker, setup walkthrough) as UNBUILT files; excluded from the crate package. No Rust change.

## 0.0.7
- A3 design (docs only): Android alarm app flow, setup walkthrough, emulator tests and the real-phone list; no code.

## 0.0.6
- A5: escalation state machine (sent, escalated, acked, unresolved) with an AlertChannel trait and fake-channel tests. No timers, HTTP, retries or provider code. Breaking (pre-1.0): first public API.

## 0.0.5
- A5 design (docs only): escalation to a second person, meets sweep constraints 1-8; no code.

## 0.0.4
- Patent sweep: USPTO reads of every remaining granted hit (14 patents); verdicts hold; three new design constraints. Docs only.

## 0.0.3
- Patent sweep: USPTO official-record reads for 11 more granted hits (second batch); 5 hits and 5 independents still unread, sweep stays INCOMPLETE. Docs only.

## 0.0.2
- A2 design (docs only): names the shared Ants and a stub for each; no sender code.

## 0.0.1
- Patent sweep, two passes (ROADMAP A1): 22 hits, independent claims read; INCOMPLETE, so A5 stays blocked. Docs only.

## 0.0.0
- Reserved name; charter, roadmap and patent record only.
