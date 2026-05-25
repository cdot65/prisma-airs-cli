# runtime dlp-profiles

## runtime dlp-profiles list

List DLP profiles

```text
airs runtime dlp-profiles list [options]
```

### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--output <format>` | No | `pretty` | Output format: pretty, table, csv, json, yaml |

### Examples

*Pretty output (default). Truncated — your tenant returns every predefined + custom DLP profile name.*

```bash
airs runtime dlp-profiles list
```

```text
Prisma AIRS — Runtime Configuration
Security profile and topic management


DLP Profiles:

  Bulk CCN
  CCPA
  CommonwealthAustralia-PrivAct88
  Corporate Financial Docs
  Financial Information
  GDPR
  GLBA
  HIPAA
  ... (more predefined + any custom profiles)
```

*JSON output (truncated — `id` is currently empty for every entry from this endpoint)*

```bash
airs runtime dlp-profiles list --output json
```

```text
[
  {
    "id": "",
    "name": "Bulk CCN"
  },
  {
    "id": "",
    "name": "CCPA"
  },
  {
    "id": "",
    "name": "CommonwealthAustralia-PrivAct88"
  },
  {
    "id": "",
    "name": "Corporate Financial Docs"
  }
]
```

*YAML output (truncated)*

```bash
airs runtime dlp-profiles list --output yaml
```

```text
id: ''
name: Bulk CCN
---
id: ''
name: CCPA
---
id: ''
name: CommonwealthAustralia-PrivAct88
---
id: ''
name: Corporate Financial Docs
```
