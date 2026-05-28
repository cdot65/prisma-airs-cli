---
"@cdot65/prisma-airs-cli": minor
---

Moved `airs runtime dlp-gen` into the DLP namespace as `airs runtime dlp generate`. All flags (`--types`, `--count`, `--out`, `--techniques`, `--seed`, `--output`) and behavior are preserved verbatim — only the command path changed. Update any scripts that invoked `runtime dlp-gen`.
