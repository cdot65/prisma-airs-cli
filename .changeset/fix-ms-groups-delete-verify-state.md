---
"@cdot65/prisma-airs-cli": patch
---

Fix `model-security groups delete` reporting unconditional success even though the API soft/async-deletes (the group often remains ACTIVE on a subsequent `get`). The command now re-verifies after deleting: it prints "deleted" only when the group is actually gone, otherwise reports that the delete was accepted but the group still reports its current state, pointing the user to re-check.
