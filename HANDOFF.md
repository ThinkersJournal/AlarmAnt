# HANDOFF: AlarmAnt

State: `alarmant` 0.0.1 on main (A1 sweep merged, INCOMPLETE); no code. Lane is `ciresnave-bot`; model Sonnet. FULL SPEED (read `C:\Projects\THROTTLE.md`).
Rule: ant-first (PM notice 2026-10-28): no new hand-rolled config/secrets/redaction/audit/HTTP/retry/scheduling/alert-delivery/release code without PM ack. A2 is design-only (DESIGN-A2.md) until the PM answers.

TASK 1: PR for branch `a2-design` (worktree `C:\Projects\AlarmAnt-wt-a2design`) is in gate (version 0.0.2 requested by me; PM may re-allocate). After merge: remove worktree, prune, delete remote branch via `gh api -X DELETE`.
TASK 2 (A1 completion): read the granted hits' claims from USPTO PPUBS (`https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/<no>`, render with `C:\Projects\.lane-state\venv-mupdf\Scripts\python.exe -I` pymupdf, Read PNGs). List and missing independents are in PATENT-SWEEP.md "Official-record reads". A5 stays blocked until complete.
TASK 3: A3 Android app needs a real phone: `[ASK]` to the PM when starting. Build limits: CARGO_BUILD_JOBS=2, low Gradle workers.
Design constraints: PATENT-SWEEP.md.
