---
"@cdot65/prisma-airs-cli": patch
---

Fixed `airs redteam properties add-value` returning HTTP 422 by sending `property_name`/`property_value` (matching the SDK schema) instead of `name`/`value`.
