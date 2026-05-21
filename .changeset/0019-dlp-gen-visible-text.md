---
"@cdot65/prisma-airs-cli": minor
---

`airs runtime dlp-gen`: add visible-text embedding techniques. Every format now has a `visible` technique that renders the synthetic payload as on-page/on-canvas text with foreground ≠ background (genuinely visible / OCR-able). PDF and DOCX additionally get `visible-samecolor`, which draws the text in the **same color as its background** — present and extractable, but camouflaged from the eye. This brings the corpus to 21 dirty files per run with `--types all --count 1`.
