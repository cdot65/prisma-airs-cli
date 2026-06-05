---
"@cdot65/prisma-airs-cli": patch
---

Fix `redteam prompt-sets get` aborting when the upstream `/version-info` endpoint returns 500. The set detail now renders regardless; Version Info degrades to an "unavailable" note (pretty) or is omitted (json/yaml) instead of failing the whole command.
