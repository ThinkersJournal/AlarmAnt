# HANDOFF: AlarmAnt

State: main has A1 sweep (INCOMPLETE) and DESIGN-A2.md; `alarmant` 0.0.2 on main, no code. Lane `ciresnave-bot`; model Sonnet. FULL SPEED (read `C:\Projects\THROTTLE.md`).
Rule: ant-first (PM 2026-10-28): no new hand-rolled config/secrets/redaction/audit/HTTP/retry/scheduling/alert-delivery/release code without PM ack. A2 stays design-only until the PM answers.

TASK 1 (A1 completion, branch `a1-ppubs-reads`, worktree `C:\Projects\AlarmAnt-wt-a1b`, PR in gate once opened): PATENT-SWEEP.md "Official-record reads" lists what is still unread (USPTO PPUBS). PDFs: `C:\Users\cires\AppData\Local\Temp\aa\pp2` (re-download via `https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/<no>` if gone). Render with `C:\Projects\.lane-state\venv-mupdf\Scripts\python.exe -I` pymupdf, view PNGs in small batches (the image reader rate-limits). Unread: US9467970B1 indep 1/8/15; US8576828B1 indep 11/21; US9686398, US9350851, US7606561, US8478242, US8825014; tails of US11768720, US8713118, US9549393. Mark COMPLETE only when done; no publish until then. A5 blocked until then.
TASK 2: A3 Android app needs a real phone: `[ASK]` to the PM when starting. Build limits: CARGO_BUILD_JOBS=2, low Gradle workers.
Design constraints: PATENT-SWEEP.md. Design: DESIGN-A2.md.
