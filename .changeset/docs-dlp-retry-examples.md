---
"@cdot65/prisma-airs-cli": patch
---

Re-probed the 12 DLP commands previously blocked under epic #127 against a fresh DLP-licensed tenant. Landed a curated input/output example for `runtime dlp filtering-profiles replace` (closes #189) and removed it from `.missing-allowlist`. The other 11 (#181-#188, #190-#192) still fail with the same upstream 400s (#80, #98, #99, #101) or CLI key-mangling bug (#105) on the new tenant; their `.missing-allowlist` entries and tracking links are kept and each sub-issue body is updated with fresh retry evidence.
