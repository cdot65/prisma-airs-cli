---
"@cdot65/prisma-airs-cli": patch
---

Fix `airs redteam report <jobId> --attacks` printing `undefined` for every attack's subcategory. The renderer now reads `sub_category_display_name` (preferred) with `sub_category` raw value as fallback, and prints an em-dash (`—`) when neither is present. Lifts `subCategoryDisplayName` into the normalized `RedTeamAttack` shape at the SDK boundary.
