kind: added
---
PC alarm, server side: `POST /pc/ack/:alert_id`, the PC program's Acknowledge (its one outgoing message, constraint 18), authorised by the PC key and accepted only for an alert whose PC alarm is sounding; it ends the alert for everyone. Verified by an end-to-end test (refused before the PC step, 401 for a wrong or empty key with nothing changed, 200 at the PC step, alert acked and escalator ended). The Windows program itself follows in this PR.
