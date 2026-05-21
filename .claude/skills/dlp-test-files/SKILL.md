---
name: dlp-test-files
description: Generate DLP test files for scanner-efficacy testing — clean carrier files plus "dirty" copies with synthetic sensitive data embedded via multiple hiding techniques. Use when asked to create DLP/data-loss test corpora, embed synthetic sensitive data in PDF/PNG/JPEG/SVG/DOCX, produce clean/dirty test pairs, or build files to exercise a content scanner's DLP detection. Drives the `airs runtime dlp-gen` CLI command.
---

# DLP Test-File Generation

Generate paired **clean** and **dirty** files to measure how well a content scanner (e.g.
Prisma AIRS) detects sensitive data hidden across file formats and embedding channels. All
embedded data is **synthetic** (reserved/test ranges only — never real PII).

This skill drives the `airs runtime dlp-gen` command. Do not hand-roll generators; use the CLI.

## When to use

- "Generate DLP test files / a DLP test corpus"
- "Embed (synthetic) sensitive data in a PDF / PNG / JPEG / SVG / DOCX"
- "Create clean and dirty test files for the scanner"
- Building a batch to score a DLP/guardrail scanner's detection rate

## Command

```bash
airs runtime dlp-gen [options]
```

| Option | Default | Meaning |
|--------|---------|---------|
| `--types <list>` | `all` | Comma list of `pdf,png,jpeg,svg,docx` (or `all`) |
| `--count <n>` | `1` | Clean files to generate per type |
| `--out <dir>` | `./temp` | Base output directory |
| `--techniques <list>` | `all` | `all` or comma list of technique ids (below) |
| `--seed <n>` | random | Seed the synthetic-payload RNG for reproducible runs |
| `--output <fmt>` | `pretty` | `pretty` or `json` summary |

### Examples

```bash
# Full corpus, all formats + techniques, into ./temp
airs runtime dlp-gen

# Only images, 3 each, reproducible
airs runtime dlp-gen --types png,jpeg,svg --count 3 --seed 42

# Just the PNG steganography variant, JSON summary
airs runtime dlp-gen --types png --techniques stego-lsb --output json
```

## Output layout

```
<out>/
  clean/<type>/<base>.<ext>                 # benign carriers (controls)
  dirty/<type>/<base>__<technique>.<ext>    # one per (clean file × technique)
  manifest.json                             # dirty file -> technique + embedded values
```

`manifest.json` records, for every dirty file, the technique used and the exact synthetic
values embedded — use it to score scanner hits/misses.

## Techniques per format

| Format | Technique ids |
|--------|---------------|
| PDF | `meta`, `hidden-text`, `trailer` |
| PNG | `text-chunks`, `trailer`, `stego-lsb` |
| JPEG | `exif`, `com`, `trailer` |
| SVG | `meta`, `hidden-text`, `comment` |
| DOCX | `core-props`, `hidden-run`, `visible` |

## Scoring against a scanner

After generating, scan each dirty file's content and compare against the manifest. Example
loop with `airs runtime scan` (text channels; images may need base64 submission per your
gateway):

```bash
airs runtime dlp-gen --types svg --out ./temp --seed 1 --output json
# for each dirty file, submit its content and check whether DLP fired,
# then reconcile with manifest.json entries[].values
```

Clean files in `clean/` are true-negative controls: a correctly-configured DLP profile should
**allow** them and **block** the matching dirty files.
