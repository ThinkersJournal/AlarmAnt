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

## Addendum 2026-11-21 (after pm-ants's go): heartbeat and dead-man searches

**What could not be run:** USPTO full text and Espacenet are not reachable from this lane's tools (web search and the Google Patents page reader only), so they were **not run**. This addendum is more searches of the same kind, not the owed full-text search; that stays owed.

| # | Query (web search, 2026-11-21) | Result |
|---|---|---|
| S3 | Google Patents: heartbeat signal not received within a predetermined time, monitoring server alerts administrator, watchdog | US8525664 (Tyco), US20200162434 (Arete M), EP2400392 (NCR), US20190140908, WO2013107137, CN101106535, US7788231 (last four snippets only) |
| S4 | "dead man's switch" software, periodic check-in missed, notify contact, claims | no software patent surfaced; apps and open-source projects only; US7034696 / US7119694 (marine proximity interrupter, snippet only) |

| Patent | Holder; status (Google, not official) | Independents read | Verdict |
|---|---|---|---|
| US8525664B2 supervised security heartbeat | Tyco; Active, est. 2032-01-05 | 1, 11, 14: a controller sends a heartbeat, waits for an acknowledgement, resets the interval on ack, and **sends a second heartbeat over a second, different communication network** when none arrives | outside as read: needs the second heartbeat over a different network, and a sender that waits for an ack; ours is a one-way heartbeat on one route (constraint S6) |
| US20200162434A1 secure heartbeat for unmanned vehicles | Arete M; Abandoned | 1, 6, 12, 13, 17: vehicle heartbeat with failsafe, a platform sending a failure alert **to the vehicle**, TLS channel, hybrid encryption | outside as read (and abandoned): nothing is sent back to the sender, no failsafe, no custom encryption |
| EP2400392A1 application heartbeat | NCR; Withdrawn | 1: status requests to objects of one application, an application-valid signal to an independent monitor | outside as read (and withdrawn); we do not poll internals of an application, the watched thing sends its own beat |

Not read: US20190140908, WO2013107137, CN101106535, US7788231, US7034696, US7119694.

**Result:** still no claim found that covers a missed-heartbeat page; two further searches and three independents read. That is thin, not a clear. Two product families (uptime and heartbeat monitors) exist but are not patents and I have not read their patents.

S6. The heartbeat is one-way and on one route: no acknowledgement the sender waits for, no retransmission over a second network, no failsafe action by the watched side.

## Addendum 2026-11-22: a keyword search I can run (Google Patents query endpoint), dead-man and heartbeat round 2

**What changed:** the lane found it can run Google Patents keyword searches (`patents.google.com/xhr/query`, US filings since 2000), so this round is a real keyword search, not web-search snippets. It is still **not** USPTO full text, Espacenet or PATENTSCOPE (not reachable from here), so the owed full-text search stays owed and **a person reads any hits**. Status and dates are Google's, not the official record.

| # | Query (2026-11-22) | Relevant results |
|---|---|---|
| S5 | absence of heartbeat message, monitoring service pushes a notification to an administrator's phone | none on point; hits were medical/IoT (Philips US9351640, Intel US12132609) |
| S6 | a user fails to check in in a time period, alert to an emergency contact | Masimo opioid family (the known 19/662,313 neighbours, constraints 13-16), Philips US10517479 |
| S7 | an external monitor detects failure of the alerting system and notifies a person by an independent channel | none on point (vehicle, smart watch, gateway hits) |
| S8 | a monitoring service expects a periodic ping, a missing ping triggers an alert | none on point (cloud, storage, robot hits) |
| S9 | a scheduled job's check-in ping, alert when it does not arrive | none on point |
| S10 | an on-call notification when a monitored message source stops sending | PagerDuty US20210304093 (abandoned) |

| Patent | Holder; status (Google) | Independents read | Verdict |
|---|---|---|---|
| US9351640B2 | Philips (now Bt Wearables); Active, est. 2026-12-16 | 1, 12-18: fall detection from body-worn accelerometers through a base unit, requesting help with no button press | outside as read: no motion sensing, no fall detection, no base unit |
| US10517479B2 | Philips; Expired per Google | 1, 7, 14, 20: wrist/accelerometer activity pattern sent to a second device that compares it with a reference | outside as read (and expired): no activity pattern or reference |
| US11385951B2 | McAfee (now JPMorgan Chase); Active, est. 2037-12-18 | 1, 11, 17: **train an ML reference template on heartbeat patterns** from many nodes, then flag variance and trigger a resilience response | outside as read: needs ML training and a pattern template; ours is a fixed missed-beat deadline from one source (constraint S7) |
| US20210304093A1 | PagerDuty; Abandoned | 31, 42, 47: a weighted operations-health **score** from interrupt events, with self-adjusting sub-score models | outside as read (and abandoned): no scoring, no models, no responder-pain metrics |

Not read: Pulson US8961185, UF US10137245 and US9950112, Noonlight US10728732 (emergency assistance, claims not opened), Masimo family members beyond the known readings. **Result:** across S1-S10, no claim found that covers a plain fixed-deadline missed-heartbeat page; that is what the searches showed, not a clearance, and the commercial uptime-monitor products' own patents are still unread.

S7. The watchdog's missed-beat rule is a fixed deadline per source: no learning, no baseline or pattern template, no score, no adaptive threshold.

**Gate for the watchdog code (ruling 478):** this is the search the lane can run. The USPTO/Espacenet full-text half is not reachable from here; pm-ants asked that the lane run what it can and a person read any hits. **The lane's recommendation: the watchdog may proceed inside S4, S6 and S7, if the PM and CireSnave accept the reachable search as sufficient.** The lane starts no watchdog code until the PM says so.
