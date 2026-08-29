# Flag Migration

The 2026-07 overhaul standardized flag names across every command group.
Version 4 keeps selected old spellings as hidden compatibility aliases. They
print a one-line deprecation notice on stderr and should not be used in new
scripts.

## Renames

| Old | Canonical | Where |
|-----|-----------|-------|
| `--format <fmt>` | `--output <fmt>` | `runtime topics create\|apply\|eval\|revert` |
| `--output <fmt>` / `--format <fmt>` | `--file-format <fmt>` | `redteam targets backup` |
| `--output <file>` | `--output-file <file>` | `runtime bulk-scan`, `runtime resume-poll`, `runtime topics sample`, `redteam prompt-sets download`, `redteam targets init` |
| `--input <file>` | `--file <file>` | `runtime bulk-scan` |
| `--page` / `--size` | `--limit` / `--offset` | all `runtime dlp` list commands |
| `--page` / `--page-size` | `--limit` / `--offset` | `runtime scan-logs query` |
| `--confirm` | `--force` | `redteam eula accept` |

Note on `topics create`: the old `--format terminal` value is accepted as a
synonym for `--output pretty`.

Note on pagination: the DLP and scan-logs APIs are page-based, so `--offset`
rounds down to a page boundary (`page = floor(offset / limit)`).

## Conventions going forward

- `--output <pretty|table|markdown|csv|json|yaml>` always means **format** (some
  commands support a subset).
- File destinations are `--output-file`; directories are `--output-dir`.
- Input files are `--file`; input directories are `--input-dir`.
- Pagination is `--limit`/`--offset` everywhere; paginated lists also expose
  `--all` and a `--max` traversal cap.
- Destructive confirmation bypass is `--force` everywhere.

## Version 4 additions

- Read commands use `--output pretty|table|markdown|csv|json|yaml`.
- `--all` performs complete SDK-backed page traversal and defaults to a
  10,000-record `--max` safety cap (`--max 0` removes it).
- `runtime profiles|topics list` default to the latest revision and accept
  `--all-versions`; their `get` commands accept `--revision` and
  `--all-versions`.
- Backup serialization is `--file-format json|yaml`, keeping `--output` free
  to mean presentation format throughout the CLI.
