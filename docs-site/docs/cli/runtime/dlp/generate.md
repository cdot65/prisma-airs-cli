---
sidebar_label: generate
---

# runtime dlp generate

:::info CLI 4.3.1 validation

The independently registry-installed CLI **4.3.1** passes **11/11** native public-CLI checks at **2026-09-07T10:29:23.947Z**: all five formats, 26 file signatures, manifest counts, JSON stdout, output precedence and rejection of invalid input before file creation. It uses sharp **0.35.4** / libvips **8.18.6**. Both the frozen production tree and fresh registry install report zero known advisories. This host uses its existing process-only font configuration; credential settings are unchanged, and temporary corpora are cleaned up before success is recorded.

Historical result: CLI 4.3.0 produced five valid PNG and five valid JPEG files at **2026-09-07T09:24:03.285Z**, but emitted a human-readable summary instead of JSON. That failure is retained in the release assessment; 4.3.1 fixes it.

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
| `--count <n>` | No | `1` | Positive safe integer: clean files per type |
| `--out <dir>` | No | `./temp` | Output base directory |
| `--techniques <list>` | No | `all` | all or comma list of technique ids |
| `--seed <n>` | No | — | Safe integer seed for reproducible payloads |
| `--output <format>` | No | configured, otherwise `pretty` | Summary format: pretty or json; explicit flag overrides environment/config |

### Examples

*Full corpus, reproducible seed (actual validated summary; temporary paths replaced)*

```bash
airs runtime dlp generate --types all --count 1 --seed 431 --output json
```

```json
{
  "clean": 5,
  "dirty": 21,
  "out": "<temporary-corpus>",
  "manifestPath": "<temporary-corpus>/manifest.json",
  "seed": 431,
  "byFormat": {
    "pdf": {
      "clean": 1,
      "dirty": 5
    },
    "png": {
      "clean": 1,
      "dirty": 4
    },
    "jpeg": {
      "clean": 1,
      "dirty": 4
    },
    "svg": {
      "clean": 1,
      "dirty": 4
    },
    "docx": {
      "clean": 1,
      "dirty": 4
    }
  }
}
```

*Images only, 3 of each*

```bash
airs runtime dlp generate --types png,jpeg,svg --count 3
```

*PNG LSB steganography only, JSON summary*

```bash
airs runtime dlp generate --types png --techniques stego-lsb --output json
```
