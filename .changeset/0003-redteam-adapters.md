---
"@cdot65/prisma-airs-cli": minor
---

Add `airs redteam adapter` commands (SDK 0.16.0 custom target adapters): `list`, `get`, `create`, `update`, `delete`, and `validate`. Update is read-modify-write so the upstream full-replacement PUT never silently wipes variables (secrets resent as `null` to keep stored values); secret values render as `(redacted)` keyed off `is_redacted`; `validate` preflights the network-broker channel for ONLINE status, auto-fills the required full variables array from `--adapter`, and renders `stderr`/`traceback` on failure with exit code 1.
