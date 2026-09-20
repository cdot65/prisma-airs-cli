---
"@cdot65/prisma-airs-cli": minor
---

Add `airs-cli redteam judge`, which judges red-team attack outputs with TypeSafe's Jev model and computes an independent attack success rate (Wilson intervals, expected ASR, agreement matrix against AIRS verdicts). Reads a scan JSON file or fetches a job through the SDK (`--job`), records and replays raw answers for threshold re-derivation, and writes private `results.json`, `judgments.json` and `summary.md` that are interchangeable with the `prisma-airs-asr-judge` harness skill. New optional tenant settings `typesafeApiKey`, `typesafeBaseUrl` and `typesafeModel`; `airs-cli doctor` gains `Typesafe credentials` and `Typesafe API` checks.
