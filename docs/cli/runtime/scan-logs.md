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
| `--page <n>` | No | `1` | Page number |
| `--page-size <n>` | No | `50` | Page size |
| `--output <format>` | No | `pretty` | Output format: pretty, table, csv, json, yaml |

### Examples

*Empty result for a 24-hour window. The upstream `/v1/mgmt/scanlogs` endpoint only accepts a fixed set of (interval, unit) pairs — `(1, hours)`, `(24, hours)`, `(7, days)`, `(30, days)`. Anything else returns API 400.*

```bash
airs runtime scan-logs query --interval 24 --unit hours --page-size 5
```

```text
Prisma AIRS — Runtime Configuration
Security profile and topic management

No scan logs found.
```

*JSON output (the renderer short-circuits the empty case across all formats, so JSON returns the same status line, not `[]`)*

```bash
airs runtime scan-logs query --interval 24 --unit hours --page-size 5 --output json
```

```text
No scan logs found.
```

*YAML output (same empty-state shape as JSON)*

```bash
airs runtime scan-logs query --interval 24 --unit hours --page-size 5 --output yaml
```

```text
No scan logs found.
```
