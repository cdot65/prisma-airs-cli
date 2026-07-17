---
sidebar_label: resume-poll
---

# runtime resume-poll

## runtime resume-poll

Safely continue a bulk scan from item-level state

```text
airs runtime resume-poll [options] <stateFile>
```

### Arguments

- `stateFile` (required) — path to the version 2 `.bulk-scan.json` state file created by `runtime bulk-scan`. Legacy state without prompt-to-request correlation is rejected.

### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--output-file <file>` | No | State output path | Output CSV file path |

### Examples

*Resume polling from a state file written by a prior `runtime bulk-scan` invocation. Output is text-only — there is no JSON/YAML mode. The CSV output path is controlled by `--output-file`.*

```bash
airs runtime resume-poll ~/.prisma-airs/bulk-scans/2026-07-17T12-00-00-000Z-1f0e...-bulk-scan.json --output-file resume-out.csv
```

```text
Prisma AIRS Resume Poll
Profile:  my-profile
Scan IDs: 1
Prompts:  3

Resume Poll Complete

Total     3
Blocked   0
Allowed   2
Failed    1
Output    /home/user/resume-out.csv
```

The `action` column of every row is exactly one of `allow`, `block`, or `failed` — there is no other outcome. The command exits 1 above because one row is `failed`; the two successful rows are still preserved in the CSV.

*Resume while the original job (or another resume) is still running* — the lock refuses the overlap:

```text
Error: Bulk-scan job is already active in process 48213. Wait for it to finish before resuming /home/user/.prisma-airs/bulk-scans/2026-07-17T12-00-00-000Z-1f0e...-bulk-scan.json.
```

A lock left behind by a crashed process is recovered automatically on the next invocation.

*Resume after an ambiguous submission* (network error or HTTP 5xx during a prior POST) — known results are recovered first, then the command stops without resubmitting:

```text
Error: Cannot safely resubmit prompt 4: its submission outcome is ambiguous. Known accepted results were preserved; inspect /home/user/.prisma-airs/bulk-scans/2026-07-17T12-00-00-000Z-1f0e...-bulk-scan.json before taking manual action.
```

### Recovery guarantees

State tracks each prompt's stable input index, AIRS `req_id`, status, accepted scan receipt, and result. Resume:

1. Polls accepted receipts without resubmitting them.
2. Submits only prompts whose prior request was definitively rejected and remains pending.
3. Writes a complete, input-ordered CSV projection atomically, so repeated resumes do not append duplicate rows.
4. Preserves failed or timed-out terminal results as `action=failed` rows and exits 1 if any are present.

When a prior POST ended in a network error or HTTP 5xx, its acceptance is ambiguous. Resume first recovers and writes all results associated with known accepted receipts, then reports the ambiguous item and stops. It never resubmits that item automatically. Exact-once submission cannot be guaranteed after ambiguous acceptance without server-side idempotency.

Polling honors `Retry-After` for rate limits and stops after 120 consecutive polls make no progress. The saved state and completed output remain available for another resume.

:::warning[Sensitive state]
The state file contains original prompt text. The CLI creates the default state directory with mode `0700` and state files with mode `0600`; preserve those protections when copying the file.
:::

Requires `@cdot65/prisma-airs-sdk` 0.13.2 or later. A per-state lock prevents overlapping resume
processes from submitting the same pending prompt.
