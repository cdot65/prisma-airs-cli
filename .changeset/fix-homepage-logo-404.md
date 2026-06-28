---
"@cdot65/prisma-airs-cli": patch
---

Fix the 404 on the documentation homepage logo. The hero image used a raw `<img src="/img/...">` path, which Docusaurus does not prefix with the site `baseUrl`; it now resolves the asset via `useBaseUrl` so the wordmark loads correctly under `/prisma-airs-cli/`.
