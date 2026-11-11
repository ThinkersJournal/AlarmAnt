# Patent reading for device registration (DESIGN-REGISTRATION.md)

Date: 2026-11-11. Run by the AlarmAnt lane (Sonnet 5.5). **A model reading, not legal advice and not a freedom-to-operate opinion; there is no attorney.** Method: three standard web searches (below), then the claims of the closest hits read from Google Patents with `tools/patent-claims.py` (the page text, not the official USPTO record). The sweep as a whole stays incomplete: say "outside as read", never "clear".

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

US 14/642147 (financial transactions, server registers a device and sends an opaque token), US10911908 (registration tokens for message handling), US11088807 (application-level acknowledgements), US9886574B2 and relatives (Assa Abloy, revoking a mobile credential), US20100262959 (Microsoft, revoking an app). Their relevance to revocation is by title only. **A gap: revocation that also invalidates pending acknowledgements is the least covered idea here**, and the only thing found near it is credential revocation in physical access control; its claims were not read.

## Constraints these readings add to DESIGN-REGISTRATION.md

1. `/register` never returns a push-service certificate or any per-OS credential; only an ack token.
2. No portal-login, transaction or "approve on your phone" flow.
3. The phone never reports "received" or "viewed" (existing rule); registration and rotation record only that the phone called the sender.
4. Revocation stays an operator CLI action; no alert-state-dependent device logic beyond "invalidate pending ack tokens".
5. Re-read before: a second device, phone-originated alerts, any accept/reject control.

## What the PM should know

This reading is thin by the sweep's own standard (three searches, three patents read, the rest by snippet). It found no claim reading on the designed registration flow; it did not prove there is none.
