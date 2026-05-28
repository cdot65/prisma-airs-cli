---
"@cdot65/prisma-airs-cli": patch
---

Fix `airs redteam report <jobId> --attacks` mis-labeling bypassed attacks as `BLOCKED`. The normalizer was reading a non-existent `successful` field; it now derives `successful` from the API's `threat` field, so rows with `threat: true` correctly render as `BYPASSED`.
