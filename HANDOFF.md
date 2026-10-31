# HANDOFF: AlarmAnt

Doing PM rows in order: (1) boot receiver = PR a3-boot-receiver 0.0.17 (done, awaiting gate); (2) START_REDELIVER_INTENT / process-death recovery (reuse ActiveAlarm); (3) Total Silence DND handling in SetupActivity. One PR each, [READY] with ran vs not. Emulator: AVD `alarm36` in `C:\Users\cires\lane-tools\alarmant` (needs `adb root` for non-exported service calls; BOOT_COMPLETED arrives ~90 s after boot). Rules: Sonnet, FULL SPEED, questions to the PM only.
