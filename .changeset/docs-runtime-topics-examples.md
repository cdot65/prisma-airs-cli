---
"@cdot65/prisma-airs-cli": patch
---

docs(runtime/topics): add live input/output examples for `runtime topics list`, `runtime topics get`, and `runtime topics update`. `runtime topics delete` remains on the missing-allowlist, blocked by SDK schema bug [cdot65/prisma-airs-sdk#165](https://github.com/cdot65/prisma-airs-sdk/issues/165) (delete response body is a plain string; SDK schema expects an object).
