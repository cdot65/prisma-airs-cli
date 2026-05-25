---
"@cdot65/prisma-airs-cli": patch
---

docs(cli): live input/output examples for `runtime deployment-profiles list`, `runtime dlp-profiles list`, `runtime scan-logs query`, `runtime resume-poll`. `scan-logs query` example documents the four valid `(--interval, --unit)` pairs accepted by the upstream `/v1/mgmt/scanlogs` endpoint — `(1, hours)`, `(24, hours)`, `(7, days)`, `(30, days)` — anything else returns API 400.
