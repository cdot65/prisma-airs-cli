---
"@cdot65/prisma-airs-cli": minor
---

CLI startup is ~6x faster (≈0.4s → ≈0.06s): the DLP test-file generator (sharp, pdf-lib, docx, piexifjs) now loads lazily, only when `airs runtime dlp generate` runs. Those four packages moved to optionalDependencies — installs with `--no-optional` skip ~50MB of native binaries, and `dlp generate` prints an install hint if they are absent.
