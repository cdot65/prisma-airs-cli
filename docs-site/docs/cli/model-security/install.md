---
sidebar_label: install
---

# model-security install

## model-security install

Install the model-security-client Python package from AIRS PyPI

```text
airs model-security install [options]
```

### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--extras <type>` | No | `all` | Source type extras to install (all, aws, gcp, azure, artifactory, gitlab) |
| `--dir <path>` | No | `model-security` | Directory to create the project in |
| `--dry-run` | No | — | Print the commands without executing |

### Examples

*Install with all extras (auto-detects uv or pip)*

```bash
airs model-security install
```

*Install with AWS support only*

```bash
airs model-security install --extras aws
```

*Preview commands without executing*

```bash
airs model-security install --dry-run
```

*Install into a custom directory*

```bash
airs model-security install --dir my-scanner
```
