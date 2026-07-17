---
"@cdot65/prisma-airs-cli": minor
---

Make runtime bulk scans reliable and resumable: preserve one input-ordered row per prompt using `(scan_id, req_id)` correlation, submit SDK 0.13.2 batches of up to 20 prompts, expose only `allow`, `block`, and `failed` outcomes, atomically project CSV output, lock active jobs, and prevent automatic resubmission after ambiguous POST failures.
