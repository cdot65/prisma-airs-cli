---
sidebar_label: languages
---

# redteam languages

List the tenant's supported languages and job types. Multilingual red team scans
use these language codes.

```text
airs redteam languages [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--management` | No | — | Query the management-plane endpoint instead of the data plane |
| `--output <format>` | No | `pretty` | Output format: pretty, table, csv, json, yaml |

#### Examples

```bash
airs redteam languages
airs redteam languages --output json
airs redteam languages --management
```
