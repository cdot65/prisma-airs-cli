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
airs runtime bulk-scan --profile demo --file prompts.txt --output-file results.csv
# progress lines appear on the terminal (stderr) without corrupting the CSV
```

API errors include the HTTP status when available, plus a reminder that
`--debug` captures the full (secret-redacted) request/response traffic.

## Confirmation prompts

Destructive commands (`runtime profiles delete`, `runtime topics delete`,
`runtime topics revert`, `redteam targets delete`, `runtime profiles cleanup`)
ask for interactive confirmation before deleting:

- **Interactive terminal (TTY):** a Y/N confirm prompt appears (default: No).
  Declining prints `Aborted` and exits `0` — a no-op, not a failure.
- **`--force`:** bypasses the prompt entirely (for `profiles delete` and
  `topics delete`, `--force` additionally performs the API force-delete and
  pairs with `--updated-by`).
- **Non-interactive (no TTY, e.g. CI or piped):** the command refuses without
  `--force` and exits `2` — a script must state destructive intent explicitly.
  Exception: `profiles cleanup` without `--force` stays a safe preview
  (exit `0`) so agents and JSON consumers can inspect before deleting.
