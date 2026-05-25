# runtime resume-poll

## runtime resume-poll

Resume polling for a previously submitted bulk scan

```text
airs runtime resume-poll [options] <stateFile>
```

### Arguments

- `stateFile` (required) —

### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--output <file>` | No | — | Output CSV file path |

### Examples

*Resume polling from a state file written by a prior `runtime bulk-scan` invocation. Output is text-only — there is no JSON/YAML mode. The CSV output path is controlled by `--output`.*

```bash
airs runtime resume-poll ~/.prisma-airs/bulk-scans/2026-05-25T13-55-11-105Z.bulk-scan.json --output resume-out.csv
```

```text
Prisma AIRS Resume Poll
Profile:  docs-example-profile
Scan IDs: 1
Prompts:  3

Polling for results...

Resume Poll Complete
─────────────────────────
Total:   1
Blocked: 0
Allowed: 1
Output:  resume-out.csv
```
