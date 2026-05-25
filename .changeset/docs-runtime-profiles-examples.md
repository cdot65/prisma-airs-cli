---
"@cdot65/prisma-airs-cli": patch
---

Add live input/output examples to the `airs runtime profiles` reference page. `list` and `get` now render Pretty + JSON + YAML captures from a live tenant; `create` and `update` render the pretty Profile Detail output (these commands have no `--output` flag). `delete` stays on the `.missing-allowlist` with an inline comment linking SDK tracking issue [prisma-airs-sdk#164](https://github.com/cdot65/prisma-airs-sdk/issues/164) — the management API returns a plain-string body that fails the SDK response schema; once SDK is patched the example will land.
