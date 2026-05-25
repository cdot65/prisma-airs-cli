---
"@cdot65/prisma-airs-cli": patch
---

docs(redteam/scan-lifecycle): add live input/output examples for `redteam status` and `redteam report` (two variants: full report + attack list filtered by severity). `redteam abort` remains on the missing-allowlist, blocked by tenant license restriction (see [cdot65/prisma-airs-cli#111](https://github.com/cdot65/prisma-airs-cli/issues/111)). Separately filed [cdot65/prisma-airs-cli#112](https://github.com/cdot65/prisma-airs-cli/issues/112) for `redteam report` routing DYNAMIC scans to the static endpoint and 500-ing — does not block these docs since the live example uses a STATIC scan.
