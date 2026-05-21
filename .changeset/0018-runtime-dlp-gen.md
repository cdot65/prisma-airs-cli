---
"@cdot65/prisma-airs-cli": minor
---

Add `airs runtime dlp-gen` to generate DLP test corpora: clean carrier files plus "dirty" copies with synthetic sensitive data embedded across PDF, PNG, JPEG, SVG, and DOCX via multiple hiding techniques (metadata, hidden text, container trailers, PNG chunks, LSB steganography, EXIF/COM, DOCX core-props/hidden-run). Writes `clean/`, `dirty/`, and a `manifest.json` mapping each dirty file to its technique and embedded values for scanner scoring. All values are synthetic / reserved-for-testing. Adds a companion `dlp-test-files` skill.
