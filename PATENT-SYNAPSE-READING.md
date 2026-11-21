# Patent reading for the Synapse adapter (inbound channel) and the watchdog (heartbeat dead-man)

Date: 2026-11-21. Run by the AlarmAnt lane (Sonnet 5.5). **A model reading, not legal advice and not a freedom-to-operate opinion; there is no attorney.** Claims read with `tools/patent-claims.py` (Google Patents page text, not the official USPTO record). The sweep as a whole stays incomplete; say "outside as read", never "clear". USPTO full text, Espacenet and PATENTSCOPE were not run. Covers `docs/SYNAPSE-ADAPTER.md` section 6 (PR #73). The instruction said "Part B first"; I found no definition of Part B, so I read the watchdog (surface b) first and the inbound channel (surface a) second.

## What was searched

| # | Query (web search) | Result |
|---|---|---|
| S1 | heartbeat monitor, missed heartbeat, dead-man switch, pages on-call | no patent claiming a software missed-check-in pager surfaced; hits were medical and wearable (US5228449, US3972320, US5515858, US5853005, hardware heartbeat). Commercial uptime/heartbeat products exist (not patents, claims unknown) |
| S2 | alert gateway receiving a message from a monitoring system, escalation, mobile acknowledgement, on-call paging | Qwest alert gateway family US8050281 / US9542830, Cerner US9881475 (and family US10121346, US9805573), US10275570, US5754111, Ecolink US12154426 |

## Claims read (independents)

| Patent | Holder; status (Google, not official) | What the independents need | Verdict |
|---|---|---|---|
| US8050281B2 alert gateway | Qwest; Active, est. 2028-04-17 | an **alert gateway device at a subscriber premises** with external and internal interfaces, receiving an alert message from an alert provider, **determining which of a plurality of subscriber devices should receive it**, formatting it into a message for that device; dependent 22: notify the provider if no acknowledgement arrives | outside as read for the new surfaces: our bridge runs on the Synapse machine and the server on a VPS, no gateway at a subscriber premises, no internal interface; no notice back to the sender of a missing ack (constraint 13). **Flag for a person:** the escalation chain already picks the first, then second device on a fixed timer; that existing path was read before, but this patent's "determining which device" wording is new to the reading and is not settled by me |
| US9542830B2 alert gateway | Qwest; Expired - Lifetime per Google (est. 2028) | claim 1 requires a gateway inside a network interface device housing on the premises wall | outside as read (hardware housing element) |
| US9881475B2 alert management, mobile | Cerner; Active, est. 2033-11-07 | a mobile **alert user interface** with accept, **reject**, message and forward-to-caregiver options | outside as read: AlarmAnt's phone has Acknowledge only, no reject, no message, no forward |

Not read (snippets only): US10121346, US9805573, US10275570, US5754111, US12154426 (Ecolink gateway to personal device), the medical heartbeat patents. **No claim was found or read for the heartbeat dead-man pattern itself**; two searches are thin, and mature commercial products suggest the art is crowded. This is the weakest part of the reading.

## Constraints this reading adds

S1. The bridge is not placed at the human's premises and has no per-device routing of its own; it raises one alert on the server, and the server's fixed chain is unchanged.
S2. No notice to the sender (Synapse role) that an alert was unacknowledged, undelivered or expired, and no nack (as constraint 13). The one reply is the Acknowledge.
S3. No reject, message or forward option on the phone.
S4. The watchdog pages the same single-priority alert; no levels, no status ladder, no schedule beyond the bounded manual pause (constraint 7, still flagged).
S5. Re-read before: a gateway at the human's premises, a delivery-failure report back to the sender, any phone reply other than Acknowledge, a heartbeat design with thresholds, categories or rotation.

## Open

A person should confirm this reading, and the US8050281 "which device" question. More searches on heartbeat/dead-man claims (Espacenet, USPTO full text) are owed before watchdog code. No adapter or watchdog code starts until that is answered.
