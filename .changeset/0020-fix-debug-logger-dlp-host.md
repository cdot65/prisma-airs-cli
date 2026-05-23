---
"@cdot65/prisma-airs-cli": patch
---

Fix `--debug` so it captures DLP API traffic. The fetch interceptor's host allowlist was missing `api.dlp.paloaltonetworks.com`, so `runtime dlp` commands were silently bypassing the JSONL log.
