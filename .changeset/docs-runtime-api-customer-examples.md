---
"@cdot65/prisma-airs-cli": patch
---

docs(runtime): add live input/output examples for `api-keys list` and `customer-apps list`. Tracked blockers for `api-keys create` (upstream 503, #116) and `customer-apps get` (upstream 403, #115); destructive ops (`api-keys regenerate`/`delete`, `customer-apps update`/`delete`) deferred for safety on the shared tenant.
