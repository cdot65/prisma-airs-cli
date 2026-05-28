# runtime dlp generate

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
