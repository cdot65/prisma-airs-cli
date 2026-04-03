---
"@cdot65/prisma-airs-cli": minor
---

Add `airs runtime profiles cleanup` command to remove old profile revisions, keeping only the latest per name. Supports `--force` to skip confirmation, `--updated-by` (defaults to git email), and `--output json`.
