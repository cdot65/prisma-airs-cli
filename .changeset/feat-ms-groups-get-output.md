---
"@cdot65/prisma-airs-cli": minor
---

Add `--output json|yaml` to `model-security groups get` (default `pretty`), matching its sibling `groups list`. Previously the command only printed the human layout, so its detail couldn't be piped into scripts.
