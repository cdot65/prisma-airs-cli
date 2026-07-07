---
"@cdot65/prisma-airs-cli": minor
---

Add `airs config {list,get,set,unset,path}` command group for managing `~/.prisma-airs/config.json` from the CLI: effective-config listing with per-key source (env/file/default), schema-validated `set`, round-trip-safe `unset` that preserves unknown file keys, secret masking with `--reveal` opt-out, and `PRISMA_AIRS_CONFIG_PATH` env override for the config file location.
