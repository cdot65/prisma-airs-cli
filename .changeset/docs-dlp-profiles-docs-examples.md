---
"@cdot65/prisma-airs-cli": patch
---

Add live input/output examples to the `airs runtime dlp profiles` reference page. `list` now renders Pretty + JSON + YAML captures from a live tenant; `delete` shows the CLI stub output describing the soft-delete patch idiom. `create`, `replace`, `patch`, and `get` remain on the `.missing-allowlist` with inline comments linking the upstream tracking issues (#101 for create/replace/patch, #80 for get) — examples will land when the upstream DLP API stops returning 400 on those endpoints.
