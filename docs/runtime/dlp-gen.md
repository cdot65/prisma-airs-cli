# DLP Test-File Generation

`airs runtime dlp-gen` generates **DLP test corpora** — clean carrier files plus "dirty"
copies with **synthetic** sensitive data embedded via multiple hiding techniques. Use it to
measure how well a content scanner detects sensitive data across file formats and channels.

!!! danger "Synthetic data only"
    Every embedded value comes from a reserved / documented test range (reserved-range SSNs,
    Luhn-valid test PANs, `example.com` emails, `555-01xx` phones, AWS `…EXAMPLE` keys). No
    real PII is ever produced.

## Usage

```bash
airs runtime dlp-gen [options]
```

| Option | Default | Meaning |
|--------|---------|---------|
| `--types <list>` | `all` | Comma list of `pdf,png,jpeg,svg,docx` (or `all`) |
| `--count <n>` | `1` | Clean files per type |
| `--out <dir>` | `./temp` | Output base directory |
| `--techniques <list>` | `all` | `all` or comma list of technique ids |
| `--seed <n>` | random | Seed for reproducible payloads |
| `--output <fmt>` | `pretty` | `pretty` or `json` summary |

**Auth:** none — purely local file generation.

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
| PDF | `meta`, `hidden-text`, `trailer` |
| PNG | `text-chunks`, `trailer`, `stego-lsb` |
| JPEG | `exif`, `com`, `trailer` |
| SVG | `meta`, `hidden-text`, `comment` |
| DOCX | `core-props`, `hidden-run`, `visible` |

## Examples

```bash
# Full corpus into ./temp
airs runtime dlp-gen

# Images only, 3 each, reproducible
airs runtime dlp-gen --types png,jpeg,svg --count 3 --seed 42

# Just PNG LSB steganography, JSON summary
airs runtime dlp-gen --types png --techniques stego-lsb --output json
```
