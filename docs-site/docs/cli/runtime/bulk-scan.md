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
| `--file <file>` | Yes | — | Input file — .csv (extracts prompt column) or .txt (one per line) |
| `--output-file <file>` | No | — | Output CSV file path |
| `--session-id <id>` | No | — | Session ID for grouping scans in AIRS dashboard |
| `--batch-size <n>` | No | `25` | Prompts per sequential logical batch; must be a positive safe integer |

Each logical batch is submitted in SDK calls of at most 20 prompts, then fully polled before the next logical batch starts. Results are correlated by `(scan_id, req_id)` and written one-to-one in original input order, even when AIRS returns several prompts under one scan ID or returns rows out of order.

### Examples

*Bulk scan with default output*

```bash
airs runtime bulk-scan --profile my-profile --file prompts.txt --batch-size 25
```

```text
Prisma AIRS Bulk Scan
Profile: AI-Firewall-High-Security-Profile
Prompts: 5
Batches: 1 (size 25)

Submitting batch 1...
Scan IDs saved: /home/user/.prisma-airs/bulk-scans/2026-07-17T12-00-00-000Z.bulk-scan.json

Bulk Scan Complete
─────────────────────────
Total:   5
Blocked: 2
Allowed: 3
Failed: 0
Output:  AI-Firewall-High-Security-Profile-bulk-scan.csv
```

*Custom output path*

```bash
airs runtime bulk-scan --profile my-profile --file prompts.txt --output-file results.csv
```

### Output and exit status

The CSV is a complete projection of all resolved items, rewritten atomically after each completed batch rather than appended. Re-running `resume-poll` therefore does not duplicate rows. It contains these detector columns:

```text
topic_violation,injection,toxic_content,dlp,url_cats,malicious_code,source_code,agent
```

The `action` column is exactly one of `allow`, `block`, or `failed`. AIRS failed and timed-out terminal results are preserved as `failed` rows. The CLI keeps all successful rows but exits 1 when any row failed.

### Retry and resume behavior

Async POST calls disable SDK retries. The CLI retries only confirmed HTTP 429 responses, honors `Retry-After`, and otherwise uses bounded exponential backoff. A definitive 4xx rejection leaves the affected prompts pending for a safe later resume. A network failure or 5xx response is ambiguous—the server may already have accepted it—so the CLI records the ambiguity and never automatically resubmits those prompts.

Accepted receipts are checkpointed per prompt before polling. Polling is bounded at 120 consecutive polls without a newly resolved prompt. Resume accepted items with:

```bash
airs runtime resume-poll ~/.prisma-airs/bulk-scans/<state-file>.bulk-scan.json
```

State files include the original prompt text. The default state directory is created with mode `0700` and each state file with mode `0600`; treat them as sensitive. Exact-once submission cannot be guaranteed after an ambiguous acceptance, so resume recovers known accepted work and reports the ambiguous item for manual review instead of risking a duplicate POST.

Requires `@cdot65/prisma-airs-sdk` 0.13.2 or later.

The command holds a per-state job lock for its lifetime. A concurrent bulk/resume invocation for
the same job exits without submitting; a lock whose local owner process has died is recovered.
