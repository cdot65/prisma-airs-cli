---
"@cdot65/prisma-airs-cli": patch
---

docs(redteam-targets): live input/output examples for `redteam eula status`, `redteam eula content`, `redteam properties list`, `redteam targets get`, `redteam targets profile`, `redteam targets metadata`, `redteam targets templates`, `redteam targets init`, `redteam targets backup`, `redteam targets restore`. Remaining write ops (eula accept, registry-credentials, targets create/update/delete/probe/update-profile/validate-auth, properties create/values/add-value, instances/devices CRUD) stay on `docs/cli/examples/.missing-allowlist` with tracking issue links — tenant returns license-read-only or API 403 for all writes.
