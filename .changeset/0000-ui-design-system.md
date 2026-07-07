---
"@cdot65/prisma-airs-cli": minor
---

New CLI output design system: `ui` primitives (header, section, keyValue, table, semantic success/warn/info/error, bullets, status) with one color and glyph per meaning, documented in src/cli/renderer/DESIGN.md. Status/progress lines now have a dedicated stderr channel. Renderers migrate onto these primitives in a follow-up.
