---
"@cdot65/prisma-airs-cli": patch
---

Fix `airs model-security rule-instances list` entry in the live smoke test reference. The CLI requires a positional `<groupUuid>` argument (rule instances are scoped to a security group); the doc was missing it. Discovered during the first live run of the smoke checklist on a real tenant.
