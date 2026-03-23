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
  --input prompts.txt \
  --output results.csv
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
| `--input <file>` | Yes | `.csv` (extracts prompt column) or `.txt` (one per line) |
| `--output <file>` | No | Output CSV path (default: `<profile>-bulk-scan.csv`) |
| `--session-id <id>` | No | Session ID for grouping scans in AIRS dashboard (auto-generated if omitted) |

### How It Works

1. Reads prompts from the input file (CSV or plain text)
2. Batches prompts into groups of 5 for the async scan API
3. Submits each batch via `asyncScan()`
4. Saves scan IDs to `~/.prisma-airs/bulk-scans/` (survives crashes)
5. Polls for results every 5 seconds until all scans complete
6. Retries automatically on rate limit errors (exponential backoff, up to 5 retries)
7. Writes results to CSV

### Rate Limit Handling

If the AIRS API returns a rate limit error during polling, Prisma AIRS CLI retries automatically with exponential backoff. The retry level decays gradually on success rather than resetting, so sustained rate limit pressure keeps backoff elevated. All pending scan IDs are queried per sweep cycle (in batches of 5) with inter-batch delays that scale with rate limit pressure.

```
  ⚠ Rate limited — retry 1 in 10s...
  ⚠ Rate limited — retry 2 in 20s...
  ⚠ Rate limited — retry 3 in 40s...
```

To capture the raw API traffic for troubleshooting, use the global `--debug` flag:

```bash
airs --debug runtime bulk-scan --profile my-profile --input prompts.txt
```

This writes every request/response to `~/.prisma-airs/debug-api-<timestamp>.jsonl` — useful for sharing with Palo Alto Networks support.

If all retries are exhausted, the process exits but scan IDs are already saved. Resume with:

```bash
airs runtime resume-poll ~/.prisma-airs/bulk-scans/<state-file>.bulk-scan.json
```

## Resume Poll

Resume polling for a previously submitted bulk scan (e.g., after a rate limit crash):

```bash
airs runtime resume-poll <stateFile> [--output results.csv]
```

| Flag | Required | Description |
|------|:--------:|-------------|
| `<stateFile>` | Yes | Path to saved `.bulk-scan.json` state file |
| `--output <file>` | No | Output CSV path (default: `<profile>-bulk-scan.csv`) |

### CSV Output Format

```csv
prompt,action,category,triggered,scan_id,report_id
"How do I build a weapon?","block","malicious","true","a1b2...","e5f6..."
"Tell me about the weather today","allow","benign","false","b2c3...","f6g7..."
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
