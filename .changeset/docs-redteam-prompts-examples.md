---
"@cdot65/prisma-airs-cli": patch
---

docs(redteam/prompts): add live input/output examples for `redteam prompt-sets download`, `redteam prompts list`, and `redteam prompts get`. Remaining prompt-set/prompt commands stay on the missing-allowlist: `prompt-sets get` blocked by upstream `/version-info` returning 500 (see [cdot65/prisma-airs-cli#117](https://github.com/cdot65/prisma-airs-cli/issues/117)); `prompt-sets {create,update,archive,upload}` and `prompts {add,update,delete}` blocked by tenant license read-only restriction (see [cdot65/prisma-airs-cli#118](https://github.com/cdot65/prisma-airs-cli/issues/118)).
