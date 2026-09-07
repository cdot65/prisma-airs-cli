---
sidebar_label: generate
---

# runtime dlp generate

:::warning CLI 4.3.0 verified limitation

The installed CLI's PNG/JPEG smoke produced five valid files per format (one clean and four dirty), verified at `2026-09-07T09:24:03.285Z`. Native generation passed, but `--output json` emitted a human-readable summary instead of JSON. The generated `manifest.json` is available; the summary format and optional sharp advisory still require remediation. See the [workflow details](../../../runtime/dlp/generate.md).

:::

## runtime dlp generate

Generate clean + dirty DLP test files (synthetic sensitive data) across PDF/PNG/JPEG/SVG/DOCX

```text
airs runtime dlp generate [options]
```

### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--types <list>` | No | `all` | Comma list: pdf,png,jpeg,svg,docx (or all) |
| `--count <n>` | No | `1` | Clean files per type |
| `--out <dir>` | No | `./temp` | Output base directory |
| `--techniques <list>` | No | `all` | all or comma list of technique ids |
| `--seed <n>` | No | — | Seed for reproducible payloads |
| `--output <format>` | No | `pretty` | Summary format: pretty or json |

### Examples

*Full corpus, reproducible seed*

```bash
airs runtime dlp generate --types all --seed 1
```

```text
DLP Test-File Generation
Output:   ./temp
Seed:     1
Clean:    5    Dirty: 15
Manifest: ./temp/manifest.json

  svg   clean=1 dirty=3
  png   clean=1 dirty=3
  pdf   clean=1 dirty=3
  jpeg  clean=1 dirty=3
  docx  clean=1 dirty=3
```

*Images only, 3 of each*

```bash
airs runtime dlp generate --types png,jpeg,svg --count 3
```

*PNG LSB steganography only, JSON summary*

```bash
airs runtime dlp generate --types png --techniques stego-lsb --output json
```
