---
"@cdot65/prisma-airs-cli": patch
---

The CLI Reference sidebar at https://cdot65.github.io/prisma-airs-cli/cli/ now lists `runtime dlp dictionaries`, `runtime dlp filtering-profiles`, `runtime dlp patterns`, and `runtime dlp profiles` as flat siblings under Runtime instead of collapsing them under a separate "Dlp" group. Achieved via a generated `docs/cli/SUMMARY.md` consumed by mkdocs `literate-nav`. No URLs or command structure changed.
