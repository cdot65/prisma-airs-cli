# DLP Test-File Generation

:::info CLI 4.4.0 validation

The independently registry-installed CLI **4.4.0** passes **11/11** native public-CLI checks at **2026-09-07T12:26:53.881Z**: all five formats, 26 file signatures, manifest counts, JSON stdout, output precedence and rejection of invalid input before file creation. It uses sharp **0.35.4** / libvips **8.18.6**. Both the frozen production tree and fresh registry install report zero known advisories. This host uses its existing process-only font configuration; credential settings are unchanged, and temporary corpora are cleaned up before success is recorded.

Historical result: CLI 4.3.0 produced five valid PNG and five valid JPEG files at **2026-09-07T09:24:03.285Z**, but emitted a human-readable summary instead of JSON. That failure is retained in the release assessment; 4.3.1 fixes it.

:::

`airs runtime dlp generate` generates **DLP test corpora** — clean carrier files plus "dirty"
copies with **synthetic** sensitive data embedded via multiple hiding techniques. Use it to
measure how well a content scanner detects sensitive data across file formats and channels.

:::danger[Synthetic data only]
Every embedded value comes from a reserved / documented test range (reserved-range SSNs,
Luhn-valid test PANs, `example.com` emails, `555-01xx` phones, AWS `…EXAMPLE` keys). No
real PII is ever produced.
:::

## Usage

```bash
airs runtime dlp generate [options]
```

| Option | Default | Meaning |
|--------|---------|---------|
| `--types <list>` | `all` | Comma list of `pdf,png,jpeg,svg,docx` (or `all`) |
| `--count <n>` | `1` | Positive safe integer: clean files per type |
| `--out <dir>` | `./temp` | Output base directory |
| `--techniques <list>` | `all` | `all` or comma list of technique ids |
| `--seed <n>` | random | Safe integer seed for reproducible payloads |
| `--output <fmt>` | configured, otherwise `pretty` | `pretty` or `json` summary; explicit flag overrides the tenant's `defaultOutput` and the default |

**Auth:** none — purely local file generation.

:::note[Optional dependencies]
File generation relies on `sharp`, `pdf-lib`, `docx`, and `piexifjs`, which ship as
**optionalDependencies** — every other `airs` command works without them. If your install
skipped optional deps (`--no-optional`), this command exits with an install hint:

```bash
pnpm add sharp pdf-lib docx piexifjs
```

Visible raster text also needs system fontconfig, fonts and a writable font cache. Minimal Alpine installs can use `apk add --no-cache fontconfig ttf-dejavu`; the official container includes these. An existing `FONTCONFIG_FILE` can select a font configuration for a single process. No credential configuration is needed.

Node 20.17 can emit its dependency JSON-module `ExperimentalWarning` on **stderr** while JSON stdout remains valid. The native test records that exact known warning and rejects other diagnostics; it does not disable runtime warnings. JSON modules became stable in [Node 20.18.3](https://nodejs.org/en/blog/release/v20.18.3).
:::

## Output

```
<out>/
  clean/<type>/<base>.<ext>                 # benign carriers (true-negative controls)
  dirty/<type>/<base>__<technique>.<ext>    # one per (clean file × technique)
  manifest.json                             # dirty file -> technique + embedded values
```

Use `manifest.json` to score scanner hits/misses: it lists, per dirty file, the technique and
the exact synthetic values embedded.

## Techniques

| Format | Technique ids |
|--------|---------------|
| PDF | `meta`, `hidden-text`, `trailer`, `visible`, `visible-samecolor` |
| PNG | `text-chunks`, `trailer`, `stego-lsb`, `visible` |
| JPEG | `exif`, `com`, `trailer`, `visible` |
| SVG | `meta`, `hidden-text`, `comment`, `visible` |
| DOCX | `core-props`, `hidden-run`, `visible`, `visible-samecolor` |

`visible` = rendered text with foreground ≠ background (genuinely visible / OCR-able).
`visible-samecolor` (PDF & DOCX) = rendered body text drawn in the **same color as its
background** — present and extractable, but camouflaged from the eye.

## Examples

```bash
# Full corpus into ./temp
airs runtime dlp generate

# Images only, 3 each, reproducible
airs runtime dlp generate --types png,jpeg,svg --count 3 --seed 42

# Just PNG LSB steganography, JSON summary
airs runtime dlp generate --types png --techniques stego-lsb --output json
```
