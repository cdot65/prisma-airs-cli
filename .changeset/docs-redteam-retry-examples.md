---
"@cdot65/prisma-airs-cli": patch
---

Docs: capture live `redteam` examples for `abort`, `eula accept`, `registry-credentials`, `targets create/update/probe/update-profile/validate-auth`, `prompt-sets create/update/archive/upload`, `prompts add/update`. Adds JSON/CSV fixtures under `docs/cli/examples/redteam/` and trims the `.missing-allowlist` (60 → 44 entries). Remaining redteam blockers re-pointed at fresh tracker issues: SDK empty-body validation (#168), properties list schema (#169), CLI add-value field-name mismatch (#197), CLI `--output` gap on prompt-sets get + properties values (#198).
