kind: added
---
The server's `AlertChannel` (`channel::Dispatcher`): the second contact's app push is queued and sent by `drain_second` (via `push_second`), and the PC alarm is raised on a `PcAlarmBoard` that the PC program polls at `GET /pc/alarm` with its own bearer key (constant-time compare, fixed 401, never cached). An acknowledged alert drops off the board. In-memory only: an alert already at the PC-alarm stage is not raised again after a restart. Verified by four tests; removing the key check fails the route test.
