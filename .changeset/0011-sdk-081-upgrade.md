---
"@cdot65/prisma-airs-cli": patch
---

Upgrade `@cdot65/prisma-airs-sdk` from `0.8.0` to `0.8.1`. Picks up a fix to the `TopicArraySchema.topic` field, which was strict-array in 0.8.0 but the AIRS API legitimately returns `null` for empty action buckets. SDK 0.8.0 + live tenants would fail `airs runtime profiles list` with `RESPONSE_VALIDATION` for any profile that had an empty allow or block bucket; 0.8.1 fixes this. No CLI behavior changes.
