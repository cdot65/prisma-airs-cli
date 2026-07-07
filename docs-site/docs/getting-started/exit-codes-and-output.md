# Exit Codes & Output Streams

The CLI follows one contract everywhere:

## Exit codes

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | Runtime failure — API error, network failure, missing credentials, partial batch failure |
| `2` | Usage error — invalid flag value, unparsable input file, missing required flag combination |

Scripts can rely on `2` meaning "fix the invocation" and `1` meaning "fix the
environment or retry".

## stdout vs stderr

- **stdout** carries data only: pretty layouts, tables, and the raw payload for
  `--output json|yaml|csv`.
- **stderr** carries everything else: progress/status lines, rate-limit retry
  warnings, deprecation notices, and errors.

This means machine-readable output always pipes cleanly:

```bash
airs runtime profiles list --output json | jq '.[].profile_name'
airs runtime bulk-scan --profile demo --input prompts.txt --output results.csv
# progress lines appear on the terminal (stderr) without corrupting the CSV
```

API errors include the HTTP status when available, plus a reminder that
`--debug` captures the full (secret-redacted) request/response traffic.
