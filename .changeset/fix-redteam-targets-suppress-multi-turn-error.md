---
"@cdot65/prisma-airs-cli": patch
---

Suppress the spurious `target_metadata.multi_turn_error_message` ("Multi turn configuration JSON not provided") that `airs redteam targets probe`, `create`, `update`, and `get` were showing whenever `multi_turn: false`. The field is only kept when `multi_turn: true`, where it indicates an actual misconfiguration.
