---
"@cdot65/prisma-airs-cli": patch
---

Build now bundles with tsup: dist/ went from ~200 files (2.5MB) to 5 files (355KB unpacked), with the DLP generator split into a lazy chunk. No API changes — library entry, types, and CLI bin paths are unchanged.
