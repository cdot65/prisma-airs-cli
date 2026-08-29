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

- **stdout** carries data only: pretty layouts and `table`, `markdown`, `csv`,
  `json`, or `yaml` payloads.
- **stderr** carries everything else: progress/status lines, rate-limit retry
  warnings, deprecation notices, and errors.

This means machine-readable output always pipes cleanly:

```bash
airs runtime profiles list --all --output json | jq '.[].profileName'
airs runtime bulk-scan --profile demo --file prompts.txt --output-file results.csv
# progress lines appear on the terminal (stderr) without corrupting the CSV
```

API errors include the HTTP status when available, plus a reminder that
`--debug` captures the full (secret-redacted) request/response traffic.

## Read-output formats

Every read command accepts the same six formats:

| Format | Contract |
|--------|----------|
| `pretty` | Interactive, decorated terminal view |
| `table` | Stable column projection using box-drawing tables |
| `markdown` | Stable GitHub-flavored Markdown table |
| `csv` | Stable projection with RFC 4180 quoting |
| `json` | Complete normalized records; lists are bare arrays and details are objects |
| `yaml` | The same complete normalized shape as JSON |

JSON and YAML intentionally contain more fields than the tabular projections.
DLP wire responses are converted from snake_case to the CLI's camelCase
structured boundary.

Output selection uses this precedence, from highest to lowest:

1. The read command's `--output <format>`.
2. The global `airs --output <format>` option.
3. `defaultOutput` in `~/.prisma-airs/config.json`, including its
   `PANW_CLI_OUTPUT` environment mapping.
4. `pretty`.

## Pagination and revisions

Paginated list commands expose `--limit`, `--offset`, and `--all`. `--all`
walks the API pages rather than requesting an oversized page. Traversal stops
at 10,000 records by default; pass `--max <n>` to choose another cap or
`--max 0` to remove it. Paging summaries and next-offset hints are stderr
status, so redirected stdout stays valid.

Profiles and topics return only their latest revision by default. Their list
commands accept `--all-versions`, and their detail commands accept
`--revision <n>` or `--all-versions` for historical reads.

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
