# Flag Migration (v2 → v3)

The v2.x series standardized flag names across every command group. Old
spellings keep working throughout v2 — they are hidden from `--help` and print
a one-line deprecation notice on stderr — and will be **removed in v3**.

## Renames

| Old | Canonical | Where |
|-----|-----------|-------|
| `--format <fmt>` | `--output <fmt>` | `runtime topics create\|apply\|eval\|revert`, `redteam targets backup` |
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

- `--output <pretty|table|csv|json|yaml>` always means **format** (some
  commands support a subset).
- File destinations are `--output-file`; directories are `--output-dir`.
- Input files are `--file`; input directories are `--input-dir`.
- Pagination is `--limit`/`--offset` everywhere.
- Destructive confirmation bypass is `--force` everywhere.

## Newly added

- `--output pretty|json|yaml` on `redteam prompts list|get`,
  `redteam instances get`, `redteam registry-credentials`
- `--limit`/`--offset` (client-side) on `redteam prompt-sets|properties|targets list`
