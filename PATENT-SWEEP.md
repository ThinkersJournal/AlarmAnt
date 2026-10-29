# Patent sweep: AlarmAnt 0.0.0 @ 88ad9d4 (no code yet)

Date: 2026-10-26 (first pass), 2026-10-27 (second pass); Run by: AlarmAnt lane, Sonnet 5.5; Method: AI-assisted web search, then independent claims read in full with `tools/patent-claims.py` (Google Patents pages); NOT legal advice; NOT a freedom-to-operate opinion. There is no attorney.

**Status: INCOMPLETE (second pass).** All four ideas were searched through a general web search (10 queries) plus one Google Patents query URL (Q11), after which that endpoint returned HTTP 503 and was abandoned. The USPTO full-text scan, Espacenet and WIPO were NOT run. The second pass added hits for ideas 2 (escalation) and 3 (self-test) and settled the one unresolved first-pass hit. **No escalation code (A5) may be written until this file is marked complete.** A2, A3, A4, A6 and A9 are not blocked by this sweep, but A3 must follow the design constraints below.

## Ideas swept
1. A high-priority push that starts a foreground alarm on the alarm stream, repeating until the user acknowledges.
2. Escalation: unacknowledged after N minutes, try a second person by text or call.
3. A scheduled self-test alarm that proves the whole chain; a missing acknowledgement the next morning is the failure signal.
4. Overriding silent / Do-Not-Disturb modes for this app's alert category (one-time permission walkthrough).

## Queries (source: WebSearch, US-only; date 2026-10-26)
| # | Idea | Query |
|---|---|---|
| Q1 | 1, 4 | `patent push notification alarm sounds until acknowledged overrides silent mode do not disturb mobile device` |
| Q2 | 2 | `patent escalation unacknowledged alert after interval notify second recipient on-call by text or voice call` |
| Q3 | 3 | `patent scheduled test alert verifies notification delivery chain end-to-end, missing acknowledgement indicates failure` |
| Q4 | 4 | `patent override device silent mode ringer volume for emergency alert category notification` |
| Q5 | 2 | `PagerDuty OR Opsgenie OR VictorOps patent incident escalation policy acknowledge timeout on-call schedule claims` (found product docs only, no patents; the query does not prove none exist) |
| Q6 | 1 | `patent mobile application receives high priority push notification plays alarm repeatedly until user acknowledges, full-screen alert` |

| Q7 | 2 | `PagerDuty patent "escalation" "acknowledge" incident notification claims patents.google.com` |
| Q8 | 1, 2 | `patent alert notification repeated until acknowledged escalate to next contact if not acknowledged within time mobile application on-call` |
| Q9 | 3 | `patent heartbeat test notification periodically sent to mobile device verify alerting path functional, failure to acknowledge test alerts administrator` |
| Q10 | 4 | `patent application bypass do not disturb critical alert notification category user grants permission sound regardless of silent switch` |
| Q11 | 2 | Google Patents `xhr/query`: `(alert OR alarm) (acknowledge OR acknowledgement) (escalat*) (mobile OR push)`, US grants, priority after 2010: 8398 results, top 50 titles scanned (only US9798596B2 Commvault "Automatic alert escalation" and US8494481B1 Amazon "Mobile alarm device" looked relevant; NOT yet read). Later phrase-restricted queries returned 0 results or HTTP 503. |

Positive control: each of Q1 to Q4 and Q6 returned patent pages, so the search reaches Google Patents / USPTO print pages. Q5 returned none, so the vendor angle is unsearched in effect.

## Hits
Claims read: every independent claim, in full, from Google Patents text (machine-converted; check the official record before quoting). Status is Google Patents' field, not the official record.

| # | patent | title | assignee | status | claims read | overlap | our difference | verdict |
|---|---|---|---|---|---|---|---|---|
| 1 | US9686398B2 | Prompt based alert by a user device in silent mode | Verizon | Expired (fee) | 1, 7, 14 | silent mode overridden to alert | every independent claim needs a text "prompt" detected in a message body | clear |
| 2 | US9350851B1 | Overriding volume settings on a mobile device | individual | Expired (fee) | 1, 14, 16 | sender notification with code raises volume, dismiss, acknowledgement back | claims 1 and 14 require an emergency code matched on the device and a remote-map location reply; claim 16 requires the sender device to also hold a code and be overridden by a third device. We have no code match, no location reply, no peer-to-peer reversal | clear (watch: do not add a matched emergency code plus location reply) |
| 3 | US8576828B1 | Silent/vibrate override for selected calling number and email/text originations | individual | Expired (fee); Google estimates term to 2032-07-13, so confirm lapse on the official record | 1, 11, 21 | overrides alert mode on an incoming message | all claims require a per-message-source override flag AND a start/end time window AND the current alert mode | design-around: A3 must not combine a per-sender override flag with a time window; override is per app, always on |
| 4 | US7606561B2 | Emergency ringing facility for mobile phones | Kyocera | Expired (fee); est. 2027-08-09 | 1, 6 | remote override of silent mode | requires a voice call to the handset first, no response, then an override code from a server | clear |
| 5 | US8478242B2 | Emergency ringing facility (continuation) | Kyocera | Expired (fee); est. 2029-02-24 | 1, 9 | same | requires a voice call first, then override code from an authentication server | clear |
| 6 | US9886840B2 | Guaranteed delivery of alert notifications through chain-of-command escalation | Groves Internet Consulting (Swift Weather) | Expired (fee) | 1 (only independent) | unacknowledged for a wait time, escalate to the next device, repeat until ack or chain exhausted | claim 1 also requires weather and physical-security data sources, an analytical system, supervisory tiers (lowest-tier user, up-chain users) and a web audit interface recording response times | clear as designed; A5 must be re-read against this claim before it is built |
| 7 | US20160086473A1 | same family (application) | Groves | Expired (fee) | 1 | alert queue, send, receive ack | broad application claim; superseded by the granted narrower claims of #6 | clear (superseded) |
| 8 | US8713118B2 | End to end email monitor | AT&T | Expired (fee) | 1, 7, 13 | test message whose non-arrival raises an alert | requires a test email forwarded to accounts in several domains and per-system checksums in subject fields | clear |
| 9 | US20100223285A1 | Critical test result management | individual | Abandoned | 1, 11, 16, 20 | send, wait for ack in a set time, escalate | application abandoned; no granted counterpart found (not searched for) | clear (abandoned; counterpart unchecked) |
| 10 | US9467970B1 | Robust routing and delivery of notifications | PagerDuty | Active | 1, 8, 15, 22 | synthetic test messages (dependent claims 5, 12, 19) | every independent claim needs several message providers each with a profile updated from stored performance metrics | design-around: A5/A6 must not select among providers using stored delivery-performance profiles |
| 11 | US8825014B2 | Repeat notification at a radio device | Qualcomm | Expired (fee); est. 2032-02-16 | 1, 15 | repeats an alert | requires a stored notification schedule with peak/non-peak periods and a battery-level setting that suppresses alerts | clear |
| 12 | US7982598B2 | Integrating user notifications and alerts on an electronic device | Access Co | Active; est. 2028-08-06 | 1, 8 | notifications separated into priority categories | requires notifications originating from a plurality of applications on the device, integrated and stored by priority; AlarmAnt is one app with one alert class | clear, but broadest active claim hit; do not add a multi-app notification inbox |
| 13 | US9479387B2 (granted from US20130346521A1) | Priority-based notifications for mobile devices | Salesforce | Active; est. 2033-10-02 | 1, 7, 14 (all independent, in full) | push notifications with priority and acknowledgement settings; dependent claim 4 keeps displaying until the user acknowledges; independent claim 7 has publisher metadata asking for an audio alert that rises in volume or frequency until acknowledged, or a pop-up that stays until acknowledged | claims 1 and 14 need a first and a second push, a threshold priority different from the first priority, and reordering by priority; claim 7 needs two pushes at different priorities and acknowledgements arriving second-then-first. AlarmAnt carries one priority and never reorders | design-around: one alert priority, no threshold, no reordering of alerts by priority. Closest ACTIVE hit to idea 1: re-read before any change to alert ordering or alert metadata |
| 14 | US9549393B2 | Mobile vibratory alert system logging alert transmissions and acknowledgments | Tractouch Mobile Partners | Expired (fee); Google estimates 2037-01-26, so confirm | 1, 10, 13 | server sends alert, user acknowledges, server logs acknowledgement or failure to acknowledge | every independent claim requires a vibration alert accessory paired to the communication device | clear (do not add a paired wearable accessory acknowledgement path) |
| 15 | US9137291B2 | Processing alerts and acknowledgments | BlackBerry / Malikie | Active; est. 2033-09-18 | 1, 5, 9, 13 | alert and acknowledgement | requires a stored primary-device indicator and an acknowledgement sent to an other device to stop its alert | clear (re-read if AlarmAnt ever runs on two devices of one user) |
| 16 | US12005234B2 | Backup notifications in a medical device system | Medtronic MiniMed | Active | 1, 10, 16 | first notification unacknowledged in a time frame, then a second notification | requires a medical application on a non-medical device, a check for access to UI resources, and the second notification delivered at a body-attached medical device | clear |
| 17 | US11768720B2 | Auto pause incident notification | PagerDuty | Active; est. 2042-03-04 | 1, 5, 10 | withholds then sends an alert | requires predicting transience from a rolling table of recently resolved alerts matched by normalised title | clear (do not add auto-pausing of alerts by history) |
| 18 | US10937000B2 | Realtime schedule management interface | PagerDuty | Active; est. 2033-12-25 | 1, 8, 15 | on-call schedules | requires a real-time schedule UI with epochs and restrictions; AlarmAnt has no schedule UI | clear |
| 19 | US8634877B2 | Automatic escalation/degradation of notifications of repetitive calls | IBM | Expired (fee) | 1, 10, 19 | escalates a notification mode | requires counting repeated incoming communications from a sender and a history of accepted/rejected communications | clear |
| 20 | US11855943B2, US11362974B2 | Event notifications notwithstanding a user instruction to disable | Rovi / Adeia | Active; est. 2038 | 1, 11 each | overrides do-not-disturb | override triggered by a count of communications from a group within a time threshold (US11855943), or an event trending in a social group (US11362974) | clear |
| 21 | US9398619B1 | Emergency alert notification and verification | AT&T Mobility | Expired (fee) | 1, 10, 19 | emergency alerts | claims decide whether the device is outside an alert area by counting received alert signals | clear |
| 22 | US9407756B2 | Notification acknowledgement in electronic devices | Intel | Active; est. 2033-11-20 | 1, 5, 9, 13 | acknowledging a notification | acknowledgement by matching an accelerometer/gyrometer motion recorded beforehand | clear (do not add motion-gesture acknowledgement) |

## Official-record reads (USPTO PPUBS downloadPdf, page images, 2026-10-27)
Method: `https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/<grant no>` saved with curl, last pages rendered to PNG with pymupdf, read by eye. These five are the hits whose verdict is a design-around or that are the broadest active claim; the Google Patents text matched the USPTO page for every claim viewed.

| patent | USPTO claim text viewed | matches Google Patents text? |
|---|---|---|
| US9479387B2 | claims 1 to 19 (all three independents 1, 7, 14, in full) | yes |
| US9467970B1 | claims 22 to 28 (independent 22 in full; independents 1, 8, 15 on the preceding page NOT viewed) | yes for claim 22 |
| US8576828B1 | claims 1 to 7 (independent 1 in full; independents 11 and 21 on the next page NOT viewed) | yes for claim 1 |
| US9886840B2 | claims 1 to 8 (the only independent, in full) | yes |
| US7982598B2 | claims 1 to 12 (independents 1 and 8, in full) | yes |

Second batch (2026-10-29, same method; each page image viewed, claims read against my verdict):

| patent | USPTO claim text viewed | verdict holds? |
|---|---|---|
| US9137291B2 | claims 1 to 13 (independents 1, 5, 9, 13) in full | yes: every independent needs a stored primary-device indicator and an ack sent to another device; claim 13 also a lower-latency second connection |
| US12005234B2 | claims 1 to 19 (independents 1, 10, 16) in full | yes: medical application on a non-medical device, plus second notification at a body-attached medical device |
| US10937000B2 | claims 1 to 20 (independents 1, 8, 15) in full | yes: real-time schedule UI with epochs and restrictions |
| US8634877B2 | claims 1 to 25 (independents 1, 10, 19) in full | yes: counts repeated communications from a sender and analyses an accept/reject history |
| US11855943B2 | claims 1 to 20 (independents 1, 11) in full | yes: DND override only on a count of communications from a particular group within a time threshold |
| US11362974B2 | claims 1 to 20 (independents 1, 11) in full | yes: DND override only when an event trends in a social group |
| US9398619B1 | claims 1 to 27 (independents 1, 10, 19) in full | yes: location decided by counting received emergency alert signals |
| US9407756B2 | claims 1 to 16 (independents 1, 5, 9, 13) in full | yes: ack by matching a recorded accelerometer/gyrometer motion |
| US11768720B2 | claims 1 to 10 (independents 1, 5, 10 start; claim 10 continues on a page not viewed) | yes so far: rolling table of recently resolved alerts |
| US8713118B2 | claims 1 to 10 (independents 1 and 7) | yes so far: email-system test; claims after 10 not viewed |
| US9549393B2 | claims 1 to 8 (independent 1) | yes so far: vibration alert accessory; independents 10 and 13 on a page not viewed |

Page images that I requested but could not view (the image reader was rate-limited), so they count as NOT read: US9467970B1 independents 1, 8, 15; US8576828B1 independents 11, 21; US9686398B2, US9350851B1, US7606561B2, US8478242B2, US8825014B2 (all pages); the unviewed tails of US11768720B2, US8713118B2 and US9549393B2 named in the table. The PDFs are downloaded; the next step is to render and view those pages.

## Needs review (to the PM)
None yet at `needs-review`. Open items that stop a "complete" mark:
- Not read: US8768737 (PagerDuty schedule management) and US8532627 (dynamic treatment of callers) returned HTTP 404 from the claims tool; CA 2838076 (alert and acknowledgment processing, delay timer), US9798596B2 (Commvault, automatic alert escalation), US8494481B1 (Amazon, mobile alarm device) and application US2024/0277934 (Medtronic, backup notifications) were seen but not read.
- #3, #4, #5, #11: Google marks them expired for fees while also estimating later terms; confirm on the official record if any design-around is to be relaxed.
- USPTO full-text scan (idea 2 and idea 3 especially), Espacenet and WIPO not run.
- Assignee searches for PagerDuty, Atlassian, Splunk, Pushover, Apple and Google not run.

## Design constraints this sweep imposes (until superseded by a complete sweep)
1. A3: override is per app, on whenever the app is installed; no per-sender flag combined with a time window (#3).
2. A3: no matched emergency code plus location reply in the alert protocol (#2).
3. A5/A6: no provider selection by stored delivery-performance profiles (#10); escalation chain stays flat, with no supervisory tiers or data-source analytics (#6).
4. App and queue carry one alert priority, no threshold priority, no priority reordering (#12, #13).
5. No wearable-accessory acknowledgement path (#14), no motion-gesture acknowledgement (#22), no auto-pause of alerts by history (#17).

## Limitations
AI-assisted search with a US-only general web search tool; patent text is machine-converted from Google Patents; statuses are Google's, not the official record; only independent claims were read in full (dependent claims read where quoted above); non-US patents and non-English patents were not searched; the search can miss a patent that uses different vocabulary ("page", "incident", "responder"). Not legal advice and not a freedom-to-operate opinion.
