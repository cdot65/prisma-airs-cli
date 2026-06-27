---
sidebar_label: bulk-scan
---

# runtime bulk-scan

## runtime bulk-scan

Scan multiple prompts via the async AIRS API

```text
airs runtime bulk-scan [options]
```

### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--profile <name>` | Yes | — | Security profile name |
| `--input <file>` | Yes | — | Input file — .csv (extracts prompt column) or .txt (one per line) |
| `--output <file>` | No | — | Output CSV file path |
| `--session-id <id>` | No | — | Session ID for grouping scans in AIRS dashboard |

### Examples

*Bulk scan with default output*

```bash
airs runtime bulk-scan --profile my-profile --input prompts.txt
```

```text
Prisma AIRS Bulk Scan
Profile: AI-Firewall-High-Security-Profile
Prompts: 5
Batches: 1

Submitting async scans...
Submitted 1 batch(es), polling for results...

Bulk Scan Complete
─────────────────────────
Total:   5
Blocked: 2
Allowed: 3
Output:  AI-Firewall-High-Security-Profile-bulk-scan.csv
```

*Custom output path*

```bash
airs runtime bulk-scan --profile my-profile --input prompts.txt --output results.csv
```
