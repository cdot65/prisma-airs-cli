---
"@cdot65/prisma-airs-cli": patch
---

fix(redteam): `airs redteam categories` pretty output now prints each category ID inline (e.g. `Jailbreak (JAILBREAK)`) so operators no longer need `--debug` to discover the IDs required by `airs redteam scan --type STATIC --categories '{…}'`. Fixes [#200](https://github.com/cdot65/prisma-airs-cli/issues/200).
