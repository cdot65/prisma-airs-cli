---
"@cdot65/prisma-airs-cli": patch
---

Fix: `airs redteam report <jobId>` now routes DYNAMIC jobs to the dynamic
report endpoint (`/v1/report/dynamic/{jobId}/report`) instead of the static
one, which was returning 500. Adds a dedicated dynamic report renderer
(Score, ASR, Goals, Threats, Summary).
