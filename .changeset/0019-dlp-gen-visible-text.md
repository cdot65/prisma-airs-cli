---
"@cdot65/prisma-airs-cli": minor
---

`airs runtime dlp generate` now emits visible-text variants alongside the existing
hidden-channel techniques: `visible` for PDF/PNG/JPEG/SVG/DOCX, plus
`visible-samecolor` for PDF and DOCX (body text drawn in the same color as its
background — extractable but camouflaged). Full corpus grows from 15 to 21
dirty files per run.
