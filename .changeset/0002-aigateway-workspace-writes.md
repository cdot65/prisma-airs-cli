---
"@cdot65/prisma-airs-cli": minor
---

Add AI Gateway workspace write commands: `airs aigateway workspace create` (requires `--name` + `--scope-name`, warns when the scope looks unrelated to the name), `update <ref>` (partial patch), and `delete <ref>` (soft delete — reported as "archived", confirmation-gated with `--force` bypass). Create/update render from a follow-up read because the API echoes partial or empty write responses.
