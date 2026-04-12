---
"@cdot65/prisma-airs-cli": minor
---

Move backup/restore from top-level to `airs redteam targets backup/restore`. Backup now captures all target fields (routing tuple, auth config, network broker UUID, extra info). Restore supplies routing defaults for targets with null routing fields. Strip null values from backup output.
