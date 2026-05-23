---
"@cdot65/prisma-airs-cli": minor
---

Add `airs runtime dlp` command group for full DLP CRUD across four subclients:
filtering-profiles (list/get/replace), patterns (full CRUD + soft-delete),
profiles (no delete — patch profile_status), and dictionaries (multipart upload,
200/204 replace handling). Bumps `@cdot65/prisma-airs-sdk` pin to `^0.9.0`.
Adds optional `PANW_DLP_ENDPOINT` env var (defaults to SDK built-in).
