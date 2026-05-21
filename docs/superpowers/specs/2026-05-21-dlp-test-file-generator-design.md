# DLP Test-File Generator — Design

**Date:** 2026-05-21
**Status:** Approved (brainstorm)
**Topic:** `airs runtime dlp-gen` + `.claude/skills/dlp-test-files`

## Purpose

Provide a repeatable way to generate **DLP test corpora**: pairs of clean carrier files and
"dirty" copies with synthetic sensitive data embedded via multiple hiding techniques. Used to
measure how well a content scanner (Prisma AIRS) detects sensitive data across file modalities
and embedding channels.

Two deliverables:

1. **Engine** — a new CLI subcommand `airs runtime dlp-gen` (TypeScript), with testable core
   logic under `src/dlp/`.
2. **Skill** — `.claude/skills/dlp-test-files/SKILL.md`, the repo's first committed skill. A
   thin guide that teaches Claude when/how to run the subcommand; it does **not** reimplement
   logic.

## User-facing command

```
airs runtime dlp-gen [options]
  --types <list>         Comma list of pdf,png,jpeg,svg,docx (default: all)
  --count <n>            Clean files to generate per type (default: 1)
  --out <dir>            Base output directory (default: ./temp)
  --techniques <list>    all | comma list of technique ids (default: all)
  --seed <n>             Seed the synthetic-payload RNG for reproducible runs (optional)
  --output <format>      Summary format: pretty (default) | json
```

**Auth:** none (purely local file generation; no AIRS API calls).

## Output layout

```
<out>/
  clean/
    pdf/<base>.pdf   png/<base>.png   jpeg/<base>.jpg   svg/<base>.svg   docx/<base>.docx
  dirty/
    pdf/<base>__<technique>.pdf       # one file per (clean file × technique)
    png/<base>__<technique>.png
    ...
  manifest.json
```

`manifest.json` records, per dirty file: source clean file, format, technique id, and the exact
synthetic values embedded (category → value). This lets a later step score scanner hits/misses.

`<base>` is a short random slug (or seed-derived). Re-running with the same `--seed` reproduces
identical files and payloads.

## Architecture (modules)

All core logic is framework-free and unit-testable under `src/dlp/`:

```
src/dlp/
  payload.ts          # synthetic PII generator (seeded RNG)
  lorem.ts            # lorem ipsum text generator
  manifest.ts         # manifest record types + writer
  generate/           # CLEAN file generators
    pdf.ts  docx.ts  png.ts  jpeg.ts  svg.ts
  embed/              # technique implementations (clean bytes + payload -> dirty bytes)
    pdf.ts  docx.ts  png.ts  jpeg.ts  svg.ts
  index.ts            # orchestration: generate -> embed (per technique) -> write + manifest
```

CLI wiring lives in `src/cli/commands/runtime.ts` (register `dlp-gen` under the existing
`runtime` group) with a small renderer in `src/cli/renderer/runtime.ts`. CLI files under
`src/cli/**` are excluded from coverage by the existing config; the testable logic is in
`src/dlp/**`.

## Clean-file generation

| Format | Library | Content |
|--------|---------|---------|
| PDF | `pdf-lib` | Multi-paragraph lorem ipsum, 1–2 pages |
| DOCX | `docx` | Lorem ipsum heading + paragraphs |
| PNG | `sharp` | Generated raster (gradient + shapes) |
| JPEG | `sharp` | Generated raster (gradient + shapes) |
| SVG | (hand-built XML) | Valid vector (shapes + title/desc) |

## Embedding techniques (dirty variants)

Each technique has a stable `id` usable in `--techniques`.

| Format | id | Technique |
|--------|----|-----------|
| PDF | `meta` | Document info dict (Title/Subject/Keywords) |
| PDF | `hidden-text` | Invisible text (render mode 3; fallback white 1pt) |
| PDF | `trailer` | Bytes appended after `%%EOF` |
| PNG | `text-chunks` | `tEXt` / `zTXt` / `iTXt` chunks |
| PNG | `trailer` | Bytes appended after `IEND` |
| PNG | `stego-lsb` | LSB steganography (length-prefixed) |
| JPEG | `exif` | EXIF `UserComment` / `ImageDescription` |
| JPEG | `com` | JPEG `COM` comment segment |
| JPEG | `trailer` | Bytes appended after `EOI` |
| SVG | `meta` | `<metadata>` / `<desc>` / `<title>` |
| SVG | `hidden-text` | `<text>` with opacity-0 / off-canvas |
| SVG | `comment` | XML comment |
| DOCX | `core-props` | Core/app document properties |
| DOCX | `hidden-run` | Hidden run (vanish + white) |
| DOCX | `visible` | Visible body line |

## Synthetic payload generator

`payload.ts` emits clearly-synthetic, validation-shaped values from reserved/test ranges,
seeded for reproducibility:

- **SSN** — reserved ranges that never get issued
- **Credit card** — Luhn-valid numbers from documented network *test* BINs (e.g. 4111-series)
- **Email** — `*@example.com|org|net`
- **Phone** — `555-0100`–`555-0199` fictional block
- **Credentials** — AWS `…EXAMPLE` access key + secret-shaped value; **no Stripe `sk_live_`
  pattern** (to avoid GitHub push-protection on committed fixtures)
- **Identity** — invented passport number + DOB

Never emits real PII. Each dirty file gets a randomized subset/instance; manifest records exact
values.

## The skill

`.claude/skills/dlp-test-files/SKILL.md`:

- **Frontmatter:** name + description with trigger phrases ("generate DLP test files", "embed
  sensitive data in PDF/PNG/JPEG/SVG/DOCX", "create clean/dirty test corpus").
- **Body:** when to use; the `airs runtime dlp-gen` invocation patterns; the technique table;
  output layout; how to feed `dirty/` files + `manifest.json` into a scan/score loop. Points at
  the CLI; no logic duplication.

## Dependencies (new)

`pdf-lib`, `docx`, `sharp`, `piexifjs`. (`sharp` ships a native binary; acceptable.)

## Testing strategy

Vitest unit tests under `src/dlp/`:

- `payload`: seeded determinism; SSN/PAN format + Luhn validity; no real-PII patterns.
- each `embed/*`: after embedding, the synthetic values are recoverable by the matching
  extractor (e.g. parse PNG chunks, read EXIF, decode LSB, grep SVG/XML, unzip docx XML), and
  the carrier remains structurally valid (re-parses).
- `generate/*`: output is a structurally valid file of the right type.
- orchestration: correct clean/dirty/manifest layout; `--seed` reproducibility.

Meets repo coverage thresholds via `src/dlp/**` (CLI wiring excluded).

## Non-goals / out of scope

- **IPTC** embedding for JPEG (needs exiftool); JPEG covered by exif + com + trailer.
- Real PII of any kind.
- Submitting files to AIRS or scoring detection (separate workflow; manifest enables it).
- XMP for images (EXIF/text-chunks cover the metadata channel).

## Open questions

- `dirty/` filename scheme: `<base>__<technique>.<ext>` — OK, or include format dir only?
- Should `manifest.json` also store base64 of each dirty file, or paths only? (Default: paths.)
- Default `--count` of 1 acceptable, or higher?
