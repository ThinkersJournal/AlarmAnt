# Patent sweep: AlarmAnt 0.0.0 @ 88ad9d4 (no code yet)

Date: 2026-10-26; Run by: AlarmAnt lane, Sonnet 5.5; Method: AI-assisted web search, then independent claims read in full with `tools/patent-claims.py` (Google Patents pages); NOT legal advice; NOT a freedom-to-operate opinion. There is no attorney.

**Status: INCOMPLETE (first pass).** All four ideas were searched, but only through a general web search (6 queries in total). The USPTO full-text scan route, Espacenet and WIPO were NOT run, and no assignee-specific search (PagerDuty, Atlassian/Opsgenie, Splunk/VictorOps, Pushover, Apple, Google) found a patent. **No escalation code (A5) may be written until this file is marked complete.** A2, A3, A4, A6 and A9 are not blocked by this sweep, but A3 must follow the design constraints below.

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
| 13 | US20130346521A1 | Priority-based notifications for mobile devices | Salesforce | Active application; est. 2033-10-02 | 1, 7, 14 | two push priority levels; dependent claims 4 to 5 continue display until user acknowledges, then tell the publisher | claim 1 needs a first and a second push with the second higher priority, displayed first; AlarmAnt has one priority | unresolved: granted counterpart not identified and its granted claims unread. Design-around until read: no multi-level priority ordering of alerts in the app or queue |

## Needs review (to the PM)
None yet at `needs-review`. Open items that stop a "complete" mark:
- #13: find the granted patent for US20130346521A1 and read its granted independent claims.
- #3, #4, #5, #11: Google marks them expired for fees while also estimating later terms; confirm on the official record if any design-around is to be relaxed.
- USPTO full-text scan (idea 2 and idea 3 especially), Espacenet and WIPO not run.
- Assignee searches for PagerDuty, Atlassian, Splunk, Pushover, Apple and Google not run.

## Design constraints this sweep imposes (until superseded by a complete sweep)
1. A3: override is per app, on whenever the app is installed; no per-sender flag combined with a time window (#3).
2. A3: no matched emergency code plus location reply in the alert protocol (#2).
3. A5/A6: no provider selection by stored delivery-performance profiles (#10); escalation chain stays flat, with no supervisory tiers or data-source analytics (#6).
4. App and queue carry one alert priority (#12, #13).

## Limitations
AI-assisted search with a US-only general web search tool; patent text is machine-converted from Google Patents; statuses are Google's, not the official record; only independent claims were read in full (dependent claims read where quoted above); non-US patents and non-English patents were not searched; the search can miss a patent that uses different vocabulary ("page", "incident", "responder"). Not legal advice and not a freedom-to-operate opinion.
