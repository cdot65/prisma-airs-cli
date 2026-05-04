---
"@cdot65/prisma-airs-cli": patch
---

Upgrade `@cdot65/prisma-airs-sdk` from `0.7.1` to `0.8.0`. SDK adds runtime Zod validation on every response and tightens `ScanResponse`/`CustomTopic` field requiredness. No CLI behavior changes — public surface is unchanged. Test fixtures updated to match the now-required `revision`/`description`/`examples` fields on `CustomTopic` responses.
