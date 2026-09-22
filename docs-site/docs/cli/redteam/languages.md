---
sidebar_label: languages
---

# redteam languages

List the tenant's supported languages and job types. Multilingual red team scans
use these language codes.

```text
airs-cli redteam languages [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--management` | No | — | Query the management-plane endpoint instead of the data plane |
| `--output <format>` | No | Resolved | Output format: pretty, table, markdown, csv, json, yaml |

#### Examples

```bash
airs-cli redteam languages
airs-cli redteam languages --output json
airs-cli redteam languages --management
```
