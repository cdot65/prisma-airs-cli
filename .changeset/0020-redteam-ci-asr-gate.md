---
"@cdot65/prisma-airs-cli": patch
---

CI: red team scan workflow now supports CUSTOM scans with `prompt_sets` from
target configs and fails the build when any target's ASR exceeds the
`ASR_THRESHOLD` (default 10%).
