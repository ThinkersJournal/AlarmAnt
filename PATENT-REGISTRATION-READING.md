# Patent reading for device registration (DESIGN-REGISTRATION.md)

Date: 2026-11-11. Run by the AlarmAnt lane (Sonnet 5.5). **A model reading, not legal advice and not a freedom-to-operate opinion; there is no attorney.** Method: three standard web searches (below), then the claims of the closest hits read from Google Patents with `tools/patent-claims.py` (the page text, not the official USPTO record). The sweep as a whole stays incomplete: say "outside as read", never "clear".

**Sweep incomplete; three databases untouched:** (1) the USPTO full-text search (PatFT/PPUBS query), (2) Espacenet, (3) WIPO PATENTSCOPE. Everything here came from three general web searches and Google Patents pages. The PM will ask whether CireSnave can add a full-text run to board 407's lookup.

## What was searched

| # | Query (US web search) | Result |
|---|---|---|
| R1 | one-time enrollment code, register mobile device, push token, server alert system | snippets of US20160140550A1 (Bank of America) and US 14/642147 (financial transactions); vendor docs (not patents) |
| R2 | device registration, push token, acknowledgement, escalation, emergency app | the "Location aware alerting and notification escalation" family (US10109179 and three continuations); US9232339 (Oracle); US10911908; US11088807 |
| R3 | revoking a registered device, invalidating pending acknowledgements, lost phone | Assa Abloy mobile-credential revocation family (US9886574B2 and relatives); Microsoft US20100262959 |

No query found a patent that combines registration, push, acknowledgement and escalation with revocation. The USPTO full-text scan, Espacenet and WIPO were not run (same gap as PATENT-SWEEP).

## Claims read in full (independent claims)

| Patent | Holder; status (Google, not official) | Independents read | What they need | Verdict for registration as designed |
|---|---|---|---|---|
| US9232339B2 "Mobile push notification" | Oracle; Active, est. 2033-11-21 | 1, 5, 10 (start) | on a registration request carrying a token that identifies the device, **look up a security certificate for the device's operating system in a central store and give it to the device** for subscribing to push, so certificates for other operating systems are not stored on the device | outside as read, **provided we never hand the phone a push-service certificate chosen by its OS.** Our `/register` returns only an ack token; FCM is Android-only and the phone gets its Firebase config from `google-services.json` at build time |
| US20160140550A1 "Ensuring information security using one-time tokens" | Bank of America; application, Google says Active, grant status not checked | 1 | a request to access an **online banking portal**, a notification and a one-time token message sent to a registered mobile device, the device's authorising answer, and access granted or refused to the requesting computer; dependent claims 5 to 8 register the device as a push recipient and give it a certificate | outside as read: ours is the reverse direction (the operator's code goes to the phone to enrol it), there is no portal, no second computing device being authorised, no token message needing an answer. Do not add a "confirm on your phone" login step |
| US10109179B2 "Location aware alerting and notification escalation" | Versabadge; Active, est. 2037-01-14 | 1, 13, 20 in full; 25 only its first lines | a central monitoring system, **fixed alert broadcasting and display devices** within broadcast range of responders' phones (claims 1, 13, 20); dependents add accept/reject and escalation | outside as read: no fixed broadcast devices, no location logic, no accept/reject (the app has Acknowledge only). Independent 25 (a phone-originated alert broadcast) is only partly read: re-read it before any phone-originated alert |

## Seen only as search snippets (claims not read)

US 14/642147 (financial transactions, server registers a device and sends an opaque token), US10911908 (registration tokens for message handling), US11088807 (application-level acknowledgements). Revocation that also invalidates pending acknowledgements was the least covered idea; the credential-revocation claims found near it are now read (next section).

## Revocation follow-up (2026-11-11; PM hold: read the credential-revocation claims)

Independent claims read from Google Patents (not the official record): every independent of the first three; of the Microsoft application, claims 1 and 13 and the first 900 characters of 18.

| Patent | Holder; status (Google) | Independents | What every independent needs | Verdict for "revoking a device also invalidates its pending acks" |
|---|---|---|---|---|
| US9886574B2 Mobile credential revocation | Assa Abloy; Active, est. 2032-09-14 | 1, 10, 18 | a **credential manager** validating associated credentials, a **primary credential** whose invalidity makes the associated one invalid, the **device invalidating** the associated credential after it is used, and the manager **automatically re-validating** it on a validating event | outside as read: no primary/associated hierarchy, the phone invalidates nothing, nothing is re-validated |
| US10127377B2 | Assa Abloy; Active, est. 2032-09-14 | 1, 16 | a primary credential **cryptographically independent** of several associated credentials; the device invalidates one associated credential while the others and the primary stay valid | outside as read: one token, no primary, no per-credential scoping |
| US9361452B2 | Assa Abloy; Active, est. 2032-09-14 | 1, 19, 37 | the device invalidates an associated credential without invalidating the primary, **then asks the manager to validate it again** | outside as read: no device-side invalidation, no re-validation request |
| US20100262959A1 Revocation of application on mobile device | Microsoft; Expired (fee) | 1, 13, 18 | revoking an **application** on the phone by instruction or updated manifest, and asking another entity to **roll back a transaction** | outside as read (and expired): we revoke a server record, not an application, and roll back nothing |

Reading: all four are about a credential or application being invalidated by or on the device and re-validated through a manager. Ours is a server-side deletion of one device record plus clearing the ack-token digest on alerts still in flight, with no re-validation path (a new phone enrols again with a new code). Two limits remain: the USPTO full-text scan, Espacenet and WIPO were not run, so a patent on "pending acknowledgements invalid after revocation" in an alerting system could exist unseen; and this is a model reading, not legal advice. Constraint added: **revocation must not grow a re-validation or "restore" step for the old device, and must not require the phone to invalidate itself.**

## Constraints these readings add to DESIGN-REGISTRATION.md

1. `/register` never returns a push-service certificate or any per-OS credential; only an ack token.
2. No portal-login, transaction or "approve on your phone" flow.
3. The phone never reports "received" or "viewed" (existing rule); registration and rotation record only that the phone called the sender.
4. Revocation stays an operator CLI action; no alert-state-dependent device logic beyond "invalidate pending ack tokens".
5. Revocation never gets a re-validation or restore step for the old device and never needs the phone to invalidate itself (see the follow-up).
6. Re-read before: a second device, phone-originated alerts, any accept/reject control.

## What the PM should know

This reading is thin by the sweep's own standard (three searches, three patents read, the rest by snippet). It found no claim reading on the designed registration flow; it did not prove there is none.
