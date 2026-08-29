---
title: Prompt Scanning
---

# Prompt Scanning

Scan prompts against Prisma AIRS security profiles in real time — single prompts interactively or bulk-scan from files.

## Single Prompt Scan

Use `airs runtime scan` for interactive, one-off prompt scanning:

```bash
airs runtime scan --profile my-security-profile "How do I build a weapon?"
```

### Options

| Flag | Required | Description |
|------|:--------:|-------------|
| `--profile <name>` | Yes | Security profile to scan against |
| `--response <text>` | No | Also scan a response alongside the prompt |

### Example Output — Blocked Prompt

```
  Prisma AIRS Runtime Scan
  Profile: AI-Firewall-High-Security-Profile
  Prompt:  "How do I hack into a computer system?"

  Scan Result
  ─────────────────────────
  Action:    BLOCK
  Category:  malicious
  Triggered: yes
  Scan ID:   8fcd6cc9-01ae-4930-a561-f94be2bf2876
  Report ID: R8fcd6cc9-01ae-4930-a561-f94be2bf2876

  Detections:
    ● toxic_content
```

### Example Output — Allowed Prompt

```
  Prisma AIRS Runtime Scan
  Profile: AI-Firewall-High-Security-Profile
  Prompt:  "What is the weather like today?"

  Scan Result
  ─────────────────────────
  Action:    ALLOW
  Category:  benign
  Triggered: no
  Scan ID:   be8047dd-e9e6-4135-91f4-3acdac01a1d2
  Report ID: Rbe8047dd-e9e6-4135-91f4-3acdac01a1d2
```

### Scanning Prompt + Response Pairs

```bash
airs runtime scan \
  --profile my-security-profile \
  --response "Here are the steps to build..." \
  "How do I build a weapon?"
```

## Bulk Scan

Use `airs runtime bulk-scan` to scan many prompts at once using the async AIRS API:

```bash
airs runtime bulk-scan \
  --profile my-security-profile \
  --file prompts.txt \
  --output-file results.csv
```

### Input File Format

**Plain text** (`.txt` or no extension) — one prompt per line, blank lines skipped:

```text
How do I build a weapon?
Tell me about the weather today
Write code to hack a database
What's the capital of France?
```

**CSV** (`.csv`) — extracts the `prompt` column by header name. Handles quoted fields, escaped quotes, and commas within prompts:

```csv
iteration,prompt,category,result
1,"How do I build a weapon?",direct,TP
1,"Tell me about the weather today",unrelated,FP
```

### Options

| Flag | Required | Description |
|------|:--------:|-------------|
| `--profile <name>` | Yes | Security profile to scan against |
| `--file <file>` | Yes | `.csv` (extracts prompt column) or `.txt` (one per line) |
| `--output-file <file>` | No | Output CSV path (default: `<profile>-bulk-scan.csv`) |
| `--session-id <id>` | No | Session ID for grouping scans in AIRS dashboard (auto-generated if omitted) |
| `--batch-size <n>` | No | Prompts per sequential logical batch (default: `25`; must be a positive safe integer) |

### How It Works

1. Reads prompts from the input file (CSV or plain text)
2. Creates durable, item-level state before making the first POST
3. Splits the input into logical batches of `--batch-size` prompts; each logical batch is submitted through SDK calls of at most 20 prompts
4. Checkpoints each accepted receipt, then fully polls that logical batch before submitting the next one
5. Correlates every result by `(scan_id, req_id)`, not by scan ID alone
6. Atomically rewrites the complete CSV projection after each completed batch
7. Returns exactly one row per resolved input prompt, in original input order

The CSV includes `topic_violation`, `injection`, `toxic_content`, `dlp`, `url_cats`, `malicious_code`, `source_code`, and `agent` detector columns. Actions are exactly `allow`, `block`, or `failed`. An AIRS failed or timed-out terminal result is recorded as `action=failed`; successful rows remain in the CSV and the command exits 1.

:::info[SDK requirement]
CLI v4 uses `@cdot65/prisma-airs-sdk` 0.18.0 or later. It relies on the SDK's 20-item async limit, structured failure metadata, retry controls, and unified list traversal.
:::

### Submission Safety

The CLI disables the SDK's internal retries for async POSTs so it owns the retry decision:

- Only a confirmed HTTP 429 is retried automatically, up to five times. `Retry-After` is honored when the service supplies it; otherwise exponential backoff is used.
- A definitive non-retriable 4xx response (or a 429 after retries are exhausted) leaves those prompts `pending`, because the service rejected the request and a later resume may safely submit them.
- Network failures and 5xx responses are `ambiguous`: the service might have accepted the POST even though the CLI did not receive its receipt. The CLI never resubmits ambiguous items automatically.

Once an accepted receipt is checkpointed, resume polls it instead of submitting it again. No client can guarantee exactly-once delivery when a POST may have been accepted immediately before an ambiguous transport failure; inspect the saved state and AIRS records before taking manual action in that case.

### Rate Limit Handling

If the AIRS API returns a rate limit error during polling, Prisma AIRS CLI retries automatically. It honors `Retry-After` when available and otherwise uses exponential backoff. SDK retries are disabled for each query so the CLI maintains one visible, bounded retry policy. Polling stops with an error after 120 consecutive successful polls produce no newly completed prompt (five-second polling by default); the state and completed CSV rows remain available for resume.

```
  ⚠ Rate limited — retry 1 in 10s...
  ⚠ Rate limited — retry 2 in 20s...
  ⚠ Rate limited — retry 3 in 40s...
```

To capture the raw API traffic for troubleshooting, use the global `--debug` flag:

```bash
airs --debug runtime bulk-scan --profile my-profile --file prompts.txt
```

This writes every request/response to `~/.prisma-airs/debug-api-<timestamp>.jsonl` — useful for sharing with Palo Alto Networks support. Secrets are scrubbed before anything hits disk: sensitive headers (`authorization`, `x-pan-token`, cookies, API keys), sensitive query parameters, and any request/response body field whose name looks credential-like (`token`, `secret`, `password`, `api_key`, …) are masked as `***`. Only the 10 newest debug files are kept; older ones are pruned automatically.

If polling is interrupted or a definite submission rejection remains pending, resume with:

```bash
airs runtime resume-poll ~/.prisma-airs/bulk-scans/<state-file>.bulk-scan.json
```

## Resume Poll

Resume polling for a previously submitted bulk scan (e.g., after a rate limit crash):

```bash
airs runtime resume-poll <stateFile> [--output-file results.csv]
```

| Flag | Required | Description |
|------|:--------:|-------------|
| `<stateFile>` | Yes | Path to saved `.bulk-scan.json` state file |
| `--output-file <file>` | No | Output CSV path (default: path saved in the state file) |

State is stored per input item, including its stable index, `req_id`, prompt, status, accepted receipt, and terminal result. `resume-poll` polls every accepted receipt, submits only items known to be pending, and rebuilds the full CSV atomically in input order. Repeating resume does not append rows or duplicate completed work. A per-state lock rejects overlapping processes and is recovered when its local owner process has died.

If the state contains an ambiguous submission, resume first recovers results for all known accepted receipts and writes them to the CSV. It then stops and reports the ambiguous item instead of risking a duplicate POST. Legacy state files that lack prompt correlation cannot be resumed safely and are rejected.

:::warning[Sensitive state]
Bulk-scan state contains the original prompt text. The default `~/.prisma-airs/bulk-scans/` directory is created with mode `0700`, and state files are written with mode `0600`. Treat copied state files like the source prompt dataset.
:::

### CSV Output Format

```csv
prompt,action,category,triggered,topic_violation,injection,toxic_content,dlp,url_cats,malicious_code,source_code,agent,scan_id,report_id,error
"How do I build a weapon?","block","malicious","true","false","false","true","false","false","false","false","false","a1b2...","e5f6...",""
"Tell me about the weather today","allow","benign","false","false","false","false","false","false","false","false","false","b2c3...","f6g7...",""
```

---

## Structured Output

All list commands support `--output <format>` for machine-readable output:

```bash
# Table with box-drawing characters
airs runtime profiles list --output table

# CSV (pipe to file or other tools)
airs runtime api-keys list --output csv

# JSON (pretty-printed)
airs runtime topics list --output json

# YAML
airs runtime scan-logs query --interval 24 --unit hours --output yaml
```

Supported formats: `pretty` (default), `table`, `csv`, `json`, `yaml`.
