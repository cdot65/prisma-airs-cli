---
"@cdot65/prisma-airs-cli": minor
---

Add the `airs aigateway` command group (AI Gateway, new SDK 0.14+ domain): `workspace list` (with `--plane data|admin`, `--status active|archived`, and `--all` to merge tenant-wide active + archived reads) and `workspace get <uuid|slug>`. New optional config fields `aiGwDataEndpoint`/`aiGwAdminEndpoint`/`aiGwTokenEndpoint` (`PANW_AI_GW_*` env vars; credentials shared with `PANW_MGMT_*`). `airs doctor` now reports AI Gateway reachability, and 403s explain which SCM grant is missing (workspace-scope vs tenant-root admin).
