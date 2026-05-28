---
"@cdot65/prisma-airs-cli": patch
---

Add `--output <pretty|json|yaml>` to `airs redteam prompt-sets get` and `airs redteam properties values`. Both previously only emitted the pretty renderer, blocking `jq`-pipe workflows and the docs sidecar's pretty+json+yaml triplet requirement.
