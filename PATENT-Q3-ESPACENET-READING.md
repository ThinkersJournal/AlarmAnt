# Espacenet Q3 hits: triage and claim reading (board 490)

Not legal advice. "Outside as read" means outside on the independent-claim text I read; the sweep is incomplete and
"partial search" applies throughout. Never "clear". Google Patents text is machine-converted and its status is not the
official record. Source: `C:/Projects/.lane-state/espacenet-q3-hits.txt` (core set = 231 results, CireSnave's Espacenet
query 2026-11-22). Read 2026-11-22 by lane alarmant for pm-ants.

Stopping test (all four on one independent claim): (a) expected periodic message, (b) missed deadline, (c) alert to a
human over a channel that does not pass through the watched source, (d) fixed non-learned deadline.

## Triage of the 231 core hits (by title; not by claims)

Dropped on title as clearly another subject (no person is alerted to a missed periodic message): ECG/heart-rate/fetal/
cardiac signal analysis and pacemakers (about 45), distributed-storage / cluster / consensus / failover / replication /
HA (about 60), drones and robot swarms (about 15), vehicles, power grid, BMS, gaming machines, blockchain, TEE, security
of drones, malware heartbeat, 5G/AP/OLT/VPN plumbing (about 50), AI training, surgical robot, firing system and the rest.
Plausible set (a missed heartbeat/liveness leads to an alarm, notice or action reaching a person or an external system):
about 40 families. Batch 1 reads 20 of them. Titles of the remaining plausible families are listed under "Not yet read".

## Batch 1 (20 families). Independent claims read to about 1000 characters each.

| Number | Status (Google, not official) | Subject | Verdict |
|---|---|---|---|
| US10425316B2 (Nokia) | Active, priority 2017-09-29 | CPE heartbeat interval missed -> update status -> alarm message "informing an external system" | **needs more**: (a) (b) read; "external system" is not stated to be a person; (d) not stated; limited to customer premises equipment. Closest of batch. No (c) as read |
| US8255086B2 (Lennox) | Active, est. exp. 2029 | HVAC subnet controllers, standby takes over on missing heartbeat; device issues "heartbeat missing" alarm | outside as read (controller failover, alarm stays on the subnet, no human alert) |
| US10785350B2 (HPE) | Active | quorum persistent/non-persistent heartbeats | outside as read (cluster quorum) |
| US10425316B2 claim 21 | as above | forced inform parameters in heartbeat message | outside as read |
| US20090172155A1 (IBM) | Abandoned | client tracks missing heartbeat intervals of an EIS server, acts on policy/user exits | outside as read; not in force |
| CN107483280B (Alibaba) | Active | node list vs heartbeat log -> available/unavailable | outside as read (no alert) |
| CN106940671B (Alibaba) | Expired | heartbeat time + period < now -> restart task copy | outside as read (restart, no person) |
| CN105718819B | Active | card device heartbeat to phone unanswered -> lock secure element | outside as read (action is local power cut) |
| CN103490943A | Pending (Google) | ProfiNet heartbeat packet-loss states | outside as read |
| CN112333758B | Expired | TV gateway stability monitoring | outside as read |
| CN121872202A | Pending, priority 2026-02-10 | elevator comm failure, standby listening | outside as read (elevator signals) |
| CN121661785B | Active | gas-leak alarm over multipath mesh | outside as read (event alarm, not a missed message) |
| CN121309638A | Pending | fire/emergency IoT, threshold alarms | outside as read |
| CN122024459A | Pending | well-lid acceleration sensor and host | outside as read (first 1000 characters only) |
| CN214965523U | Expired | patient heartbeat (physiological) alarm device | outside as read |
| WO2016142960A1 | Ceased | driver cardiac arrest stops engine | outside as read |
| JP7487806B2 (Denso) | Active | tamper detection by signal deviation | outside as read |
| CN122339957A | Pending | V2X log-gap compensation | outside as read |
| US9984555B2 | Expired - fee related | handheld base, sensor module, proximity tag notifies on proximity change | outside as read (hardware-specific; proximity, not a deadline) |
| US12231923B2 (Plume) | Active, est. exp. 2038 | ML-derived Wi-Fi alarms | outside as read (learned, not fixed deadline) |
| CN117261668B | Active | charging-pile analytics | outside as read |

Not fetched: US8917610B2 (HTTP 503 after retries), CN122970019A (404 on Google). Retry next batch.

## Batch 2 (18 families fetched of 20). Independent claims read to about 900 characters each.

| Number | Status (Google, not official) | Subject | Verdict |
|---|---|---|---|
| US8917610B2 (IBM) | Expired - fee related | intermittent Ethernet link-failure counting | outside as read |
| CN111526049B (ICBC) | Active | ops workflow editor/engine | outside as read |
| CN114531379A | Pending | edge-gateway heartbeat values, missing static data upload | outside as read (data re-upload, no human alert) |
| CN120687324A | Pending | service-call-log situation awareness | outside as read |
| US11915156B1 (Splunk) | Active, est. exp. 2037 | learned leading indicators | outside as read (learned, predictive) |
| US20260147881A1 (BofA) | Pending | AI session-anomaly resolution | outside as read |
| CN112949869B | Active | predicts vehicle disconnection from heartbeat history (model) | outside as read (learned model, no human alert) |
| CN121860807A | Pending | GNN outage-area judging | outside as read |
| CN121842145A | Pending | IM connection reliability | outside as read |
| CN119437736B (Seres) | Active | UDP heartbeat sequence vehicle fault detection | outside as read (vehicle layers; no person alerted as read) |
| CN122614084A | Pending | UAV-assisted platoon rescue | outside as read |
| CN117158928A | Pending | neonate temperature/heartbeat (physiological) | outside as read |
| CN121510354A | Pending | LPWAN intercom election | outside as read |
| CN122552122A | Pending | respiratory rate from RR intervals | outside as read |
| US11406315B2 | Active | brain-trauma sensors | outside as read |
| CN110475324B (Skyworks) | Active | die-to-die heartbeat, low-power mode | outside as read |
| CN101090308B (Huawei) | Expired | dual-backup heartbeat frames | outside as read |
| (CN122970019A, CN122672037A, CN122932157A) | not on Google (404) | not read | **unread**; need Espacenet/other source |

Verdict summary for Q3 batches 1-2: no independent claim read reads on (a)-(d) together. Closest: US10425316B2 (Nokia),
which reads (a)(b) and an alarm to an "external system" but is limited to customer premises equipment and states
neither a person nor a fixed deadline: "needs more".

Unread plausible families remain (about 15 by title, listed in the next batch); this file is extended as they are read.
