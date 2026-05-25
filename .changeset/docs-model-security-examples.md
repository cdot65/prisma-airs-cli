---
"@cdot65/prisma-airs-cli": patch
---

docs(model-security): add live input/output examples for read-only commands — `groups get`, `rules get`, `rule-instances get`, `labels keys`, `labels values`, `pypi-auth`. Filed tracking issue #121 for scan-detail commands (`scans get`/`evaluations`/`evaluation`/`violations`/`violation`/`files`) that need scan fixtures on a docs-capture tenant. Destructive ops (`groups create`/`update`/`delete`, `labels add`/`set`/`delete`, `rule-instances update`, `scans create`) remain on the allowlist, deferred for safety on the shared QA tenant.
