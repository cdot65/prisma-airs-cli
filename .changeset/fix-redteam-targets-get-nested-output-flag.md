---
"@cdot65/prisma-airs-cli": patch
---

Fix `airs redteam targets get` so nested `connection_params` (`request_headers`, `request_json`, `response_json`) render as indented JSON instead of `[object Object]`, and add `--output <pretty|json|yaml>` for parity with `targets list` / `scan`.
