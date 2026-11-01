# Patent sweep: AlarmAnt 0.0.0 @ 88ad9d4 (no code yet)

Date: 2026-10-26 (first pass), 2026-10-27 (second pass); Run by: AlarmAnt lane, Sonnet 5.5; Method: AI-assisted web search, then independent claims read in full with `tools/patent-claims.py` (Google Patents pages); NOT legal advice; NOT a freedom-to-operate opinion. There is no attorney.

**Status: USPTO reads of all granted hits complete (2026-10-29); the sweep as a whole stays INCOMPLETE until the PM accepts the routes not run (Espacenet, WIPO, assignee searches) as sufficient.** All four ideas were searched through a general web search (10 queries) plus one Google Patents query URL (Q11), after which that endpoint returned HTTP 503 and was abandoned. The USPTO full-text scan, Espacenet and WIPO were NOT run. The second pass added hits for ideas 2 (escalation) and 3 (self-test) and settled the one unresolved first-pass hit. **No escalation code (A5) may be written until this file is marked complete.** A2, A3, A4, A6 and A9 are not blocked by this sweep, but A3 must follow the design constraints below.

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

Third batch (2026-10-29, same method; every claim page of each patent viewed unless noted):

| patent | USPTO claim text viewed | verdict |
|---|---|---|
| US9467970B1 | independents 1, 8, 15, 22 in full | design-around holds: every independent needs provider profiles updated from stored performance metrics |
| US8576828B1 | independents 1, 11, 21 in full | design-around holds: every independent needs (a) an override flag per message source, (b) a received-within-window test, (c) a current-alert-mode test, then override to a second mode |
| US9686398B2 | claims 1 to 20 (independents 1, 7, 14) | clear: needs a first and a second text prompt detected inside message bodies, then alerts that override silent mode |
| US9350851B1 | claims 1 to 17 (independents 1, 14, 16) | clear, but the closest silent-override hit: every independent needs a stored emergency code matched against the notification's code before the volume is raised; claims 1 and 14 add a matched remote map code and a location reply. Constraint 2 stays |
| US7606561B2 | claims 1 to 15 (independents 1, 6) | clear: needs a voice call with no answer, then a phonebook override code requested from a server over a data network |
| US8478242B2 | claims 1 to 18 (independents 1, 9) | clear: same override-code family as US7606561 |
| US8825014B2 | claims 1 to 19 (independents 1, 15) | clear: repeat annunciation gated by a peak/non-peak schedule time period and a battery charge level setting |
| US9549393B2 | claims 9 to 14 (independents 10, 13; claim 1 read in batch 1) | clear: every independent needs a vibration accessory |
| US8713118B2 | claims 11 to 18 (independent 13; claims 1 to 10 read in batch 2) | clear: needs a test email forwarded to accounts in different domains, and checksum values in subject fields |
| US11768720B2 | claims 10 to 16 (claims 1 to 10 read in batch 2) | clear: needs a pause period and a prediction model; dependents add normalised alert titles |
| US9798596B2 | claims 1 to 15 (independents 1, 6, 11) | clear: needs a backup-system operation failure with a schedule window and throughput estimate, and a directory-services availability query |
| US8494481B1 | claims 1 to 20 (independents 1, 18, 19, 20) | clear but near idea 2: needs an alarm triggered at a site, a responder's acceptance of the call message, then escalation if that responder has not cleared the alarm within a period. New constraint 6 |
| US8768737B1 | claims 1 to 30 (independents 1, 9, 17, 24) | clear: schedule layers with precedence values and epochs (PagerDuty schedules) |
| US8532627B1 | claims 1 to 18 (independents 1, 7, 13) | clear: needs a call to a silent phone, a caller-eligibility check and a caller entry marking the call important |

Constraint 6 (from US8494481B1): the escalation chain stays a single step: no alert has an "accepted" state separate from "acknowledged", and no timer that escalates after acceptance but before clearance.
Constraint 7 (from US8825014B2): a repeating alert must not be gated by a battery-level setting combined with schedule time periods.
Constraint 8 (from US9686398B2): do not trigger an alert by detecting text in a message body; the alert is a data push.

Result: the claims of every granted hit named in this file have now been read on the USPTO record, as the PM required on 2026-10-26. The Google Patents text matched the USPTO text for every claim viewed. Left undone: Espacenet, WIPO and assignee-specific searches were NOT run; the "Expired - Fee Related" entries are still unconfirmed on the USPTO record; the three published applications (CA 2838076, US2024/0277934, US20160086473) were read on 2026-10-30 from Google Patents only (see "Applications read"), not on the USPTO or CIPO record.

## Applications read (2026-10-30, Google Patents text only; not the USPTO/CIPO record)
| Document | Assignee, status | Independent claims | Verdict |
|---|---|---|---|
| US20160086473A1 | Groves Internet Consulting; Expired (fee), granted as US9886840B2 (#6) | 1 (of 2 claims): external data source, analytical system, alert queue, notification system that sends to an end user and receives an acknowledgment receipt. Read verbatim | clear (superseded by #6, which is narrower; AlarmAnt has no analytical system that creates the notification from external data) |
| US2024/0277934A1 | Medtronic Minimed; pending, so claims may still change | 1, 13, 20 (20 claims; all 20 read). Each needs a first notification unacknowledged at a mobile device within a time frame, then a second notification delivered to a MEDICAL DEVICE. Claim 1 read verbatim; 13 and 20 from a model summary of the page | clear while the second notification never goes to a medical device. New constraint 9 |
| CA 2838076 (CA2838076C granted 2021-03-09) | BlackBerry; Active, est. to 2033-12-18 | 1, 8, 16 (16 claims). Claim 1: on a notification from a server, a primary device alerts and sends an acknowledgment so the other device stops alerting; a non-primary device starts a delay timer before alerting. Claim 16: acknowledgment sent over a second lower-latency connection. Summarised by a model from Google Patents, NOT read verbatim | clear for the current design (one device per recipient, no primary/secondary). New constraint 10; read the claims verbatim on CIPO before any multi-device feature |

Constraint 9 (from US2024/0277934): A5's second notification goes to a person (push, SMS, voice), never to a medical device such as an insulin pump or glucose sensor.
Constraint 10 (from CA 2838076): no primary/secondary arrangement across one recipient's devices, with a delay timer on the secondary and an acknowledgment that silences the other device.

## Third pass (2026-10-31, Google Patents route with a positive control)
Route: `WebSearch` with `allowed_domains: [patents.google.com]`, so every result is a Google Patents page. Positive control: Q12 to Q14 returned patents this file already holds (US9798596B2, US9886840B2, US8713118B2, US9479387B2), so the route reaches patent pages and the queries ask about our ideas. Claims text: `python C:\Projects\tools\patent-claims.py <numbers>` (verbatim from Google Patents; the claim text is Google's machine conversion and statuses are Google's, not the official record). Every independent claim below was read verbatim from that output. Wording: "outside as read" means none of the independent claims, as read, covers the design in this repo; it is not a clearance and not legal advice. The summariser-only reads made earlier in this pass were discarded and redone with the script.

| Q | idea | query (patents.google.com only) |
|---|---|---|
| Q12 | 2 | `patent unacknowledged alert escalate to next contact by SMS or voice call after timeout mobile push` |
| Q13 | 3 | `patent scheduled test alert verifies notification delivery path, missing acknowledgement indicates failure` |
| Q14 | 1, 4 | `patent push notification alarm repeats until acknowledged overrides silent do not disturb mobile application` |

| # | patent | assignee, status (Google) | independents read verbatim | overlap | our difference (every independent needs it) | as read |
|---|---|---|---|---|---|---|
| 23 | US8477914B1 | Cox; active est. 2031-09-11 | 1, 15, 19 | no reply, repeated escalated messages, SMS/voice to affiliates (19) | automatically modifying a functionality of the targeted device when no reply arrives | outside; constraint 11 |
| 24 | US8648706B2 | Honeywell; active est. 2032-08-05 | 1, 11 | escalation of unacknowledged alarms | claim 1: urgency levels with threshold types; claim 11: unacknowledged / acknowledged / pending life cycle | outside; constraint 12 |
| 25 | US8364221B2 | Covidien; "expired - fee related" est. 2028-08-06 (confirm) | 1, 9, 15, 21 | unacknowledged alarm escalates its sound | a pulse oximeter with several built-in speakers | outside |
| 26 | US20050249023A1 | Philips; abandoned | 1, 8 | a remote sender overrides a user's do-not-disturb mode | abandoned, no term | outside |
| 27 | US9641669B2 | Apple; active | 1, 13, 17 | do-not-disturb | exit do-not-disturb on detected device motion | outside |
| 28 | US8762190B1, US9830578B2 | PagerDuty; active est. 2033-12-25 | 1, 9, 17, 24 (US8762190B1); 1, 8, 15, 21 (US9830578B2) | on-call schedules | schedule layers, epochs, virtual start time, real-time schedule UI | outside |
| 29 | US20130332721A1 / US9348607B2 | Apple; active est. 2034 | 1, 11, 21 / 1, 9, 17 | do-not-disturb ("restricted notification") | preventing sound/light/movement on time-, location- or movement-based criteria; the granted claims add a time period, a location and a prior-communication check; AlarmAnt never suppresses | outside |
| 30 | US8204523B1 | Cellco (Verizon); active est. 2030-12-22 | 1, 11, 24, 32, 39 | notification delivery | notifies an account holder of changes to a mobile station's account via a delivery procedure chosen by destination age | outside |
| 31 | US9288164B2 / US20140365569A1 | Apple; active est. 2034-02-17 | 1, 8, 12, 16, 21 | push, acknowledgement wait | silent notifications that modify a notification on another device of the same user via an identity service (claim 12: wait for an ack of a routed silent notification, store, reroute on device presence); AlarmAnt sends no silent notification and has one device per recipient | outside |
| 32 | US8823507B1 | Amazon; expired (fee) est. 2032-10-03 | 1, 3, 15, 18 | notification alert | choosing the device a user is interacting with, or user presence by head/face/gaze data | outside |
| 33 | US20140171146A1 | Apple; active est. 2034-12-22 | 1, 10, 19, 20, 21, 22 | alarm | sensor-based activity or sleep decides alarm behaviour or time | outside |
| 34 | US10904379B2 | Apple; active est. 2039-09-09 | 1, 16, 31 | reduced notification mode | UI to deactivate a reduced notification mode, then show notifications | outside |
| 35 | US20150106121A1 | Masimo; active (granted as US10832818B2) | 1, 7, 22 | **claim 1: alarm from a patient device, notification to a clinician device, an escalation condition that depends on whether a response arrived, then automatically sending to a second clinician device** | the published claim 1 has no medical element beyond the source of the alarm and is broader than A5's design; the granted US10832818B2 (claim 1 read verbatim) is a patient monitor with an optical scanner reading a patient bracelet and a monitor tag | outside for the granted claim, **but the published claim is the closest escalation claim found so far and no continuation of the family was searched; see "Needs review"** |
| 36 | US6237034B1, EP2393047A1, US8340702B2, US20040054743A1, US7552057B2, US20030172077A1 | various; expired, withdrawn or abandoned (Google) | all independents | acknowledgements, reminders, call alerts, contact-centre escalation, process exceptions, rule-based notification | telecom alarm acknowledgement; calendar reminders; unsuccessful-call email; contact-centre folders; process monitoring; event rules and device connectors | outside |

NOT read: US5748884A (MCI/Verizon, expired 2016-06-13; fetched, not read this pass).

Constraint 11 (from US8477914B1): a missing acknowledgement never changes the recipient's device (no lock, disable or mode change).
Constraint 12 (from US8648706B2): A5 has no urgency levels or threshold types, and no alert state after acknowledged (extends constraint 6).

## Needs review (to the PM)
None yet at `needs-review`. Open items that stop a "complete" mark:
- CA 2838076, US2024/0277934 and US20160086473 are read (Google Patents text only, 2026-10-30; see "Applications read"). Not confirmed on the USPTO/CIPO record; CA 2838076 claims are summarised, not read verbatim.
- #3, #4, #5, #11: Google marks them expired for fees while also estimating later terms; confirm on the official record if any design-around is to be relaxed.
- USPTO full-text scan, Espacenet and WIPO not run. Third pass (above) ran 3 Google Patents queries with a positive control and read the independent claims of 26 documents verbatim with patent-claims.py; US5748884A is unread. The sweep is still not complete, and nothing here clears A5.
- Domain-restricted Google Patents route (found 2026-10-31, positive control and verbatim reads): see "Third pass" below; the summariser-only notes first written here were replaced by verbatim reads there.
- Assignee searches: 3 web queries run 2026-10-31 (Opsgenie/Atlassian escalation; Splunk On-Call/VictorOps escalation; PagerDuty self-test/synthetic notification). NO CLEARANCE, NO SEARCH ROUTE REACHED: all three returned product docs and forum posts and no patent page, so the null proves nothing: the same search tool reached Google Patents for Q1 to Q4 and Q6 but not here (no positive control for assignee queries). Pushover, Apple and Google not run. These need a patent-database route (USPTO assignee field, Espacenet) the lane's tools do not reach; the PM decides whether to accept the sweep as it stands.

- Masimo family: US20150106121A1 claim 1 is a broad escalation claim; the granted US10832818B2 is narrow, but no continuation or divisional of the family was searched. A5 stays blocked until the PM accepts that or a continuation search is done.

## Design constraints this sweep imposes (until superseded by a complete sweep)
1. A3: override is per app, on whenever the app is installed; no per-sender flag combined with a time window (#3).
2. A3: no matched emergency code plus location reply in the alert protocol (#2).
3. A5/A6: no provider selection by stored delivery-performance profiles (#10); escalation chain stays flat, with no supervisory tiers or data-source analytics (#6).
4. App and queue carry one alert priority, no threshold priority, no priority reordering (#12, #13).
5. No wearable-accessory acknowledgement path (#14), no motion-gesture acknowledgement (#22), no auto-pause of alerts by history (#17).
6. A5: one acknowledgement step; no separate "accepted" state, and no timer that escalates after acceptance but before clearance (US8494481B1).
7. A5/A6: a repeating alert is not gated by a battery-level setting combined with schedule time periods (US8825014B2).
8. A2/A3: alerts are data pushes; no alert is triggered by detecting text in a message body (US9686398B2).
9. A5: the second notification goes to a person, never to a medical device (US2024/0277934).
10. A3/A5: one device per recipient; no primary/secondary delay-timer arrangement between a recipient's devices (CA 2838076).
11. A5: a missing acknowledgement never changes the recipient's device (US8477914B1).  
12. A5: no urgency levels, threshold types or post-acknowledged states (US8648706B2).

## Limitations
AI-assisted search with a US-only general web search tool; patent text is machine-converted from Google Patents; statuses are Google's, not the official record; only independent claims were read in full (dependent claims read where quoted above); non-US patents and non-English patents were not searched; the search can miss a patent that uses different vocabulary ("page", "incident", "responder"). Not legal advice and not a freedom-to-operate opinion.
