---
sidebar_label: scan-logs
---

# runtime scan-logs

## runtime scan-logs query

Query scan logs

```text
airs runtime scan-logs query [options]
```

### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--interval <n>` | Yes | — | Time interval |
| `--unit <unit>` | Yes | — | Time unit (hours) |
| `--filter <filter>` | No | `all` | Filter: all, benign, threat |
| `--limit <n>` | No | `50` | Max results per page (API page size) |
| `--offset <n>` | No | `0` | Starting offset — rounds down to a page boundary |
| `--output <format>` | No | Resolved | Output format: pretty, table, markdown, csv, json, yaml |

### Examples

*Empty result for a 24-hour window. The upstream `/v1/mgmt/scanlogs` endpoint only accepts a fixed set of (interval, unit) pairs — `(1, hours)`, `(24, hours)`, `(7, days)`, `(30, days)`. Anything else returns API 400.*

```bash
airs runtime scan-logs query --interval 24 --unit hours --limit 5 --output pretty
```

```text
Prisma AIRS — Runtime Configuration
Security profile and topic management

No scan logs found.
```

*JSON output uses the list contract even when the result is empty.*

```bash
airs runtime scan-logs query --interval 24 --unit hours --limit 5 --output json
```

```json
[]
```

*YAML output uses the same bare-array shape.*

```bash
airs runtime scan-logs query --interval 24 --unit hours --limit 5 --output yaml
```

```yaml
[]
```
