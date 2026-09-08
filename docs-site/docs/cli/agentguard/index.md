---
title: AgentGuard — AI Supply Chain
sidebar_label: Overview and examples
---

# airs agentguard

:::warning Experimental browser APIs — CLI 5.4.1 / SDK 0.29.0
AgentGuard reads undocumented browser APIs for agent and skill scanning. These commands are introduced in CLI 5.4.1 with an exact dependency on SDK 0.29.0. These commands do not create scans, upload skills or modify policies.
:::

Uses existing Management credentials from the read-only `~/.prisma-airs/config.json` or `PANW_MGMT_*` environment. No pasted browser bearer token or runtime scan API key is required. Optional configuration keys: `agentGuardDataEndpoint`, `agentGuardMgmtEndpoint`, `agentGuardTokenEndpoint`; environment equivalents are `PANW_AGENT_GUARD_DATA_ENDPOINT`, `PANW_AGENT_GUARD_MGMT_ENDPOINT`, and `PANW_AGENT_GUARD_TOKEN_ENDPOINT`.

## Read commands

```bash
airs agentguard scans list --output json
airs agentguard scans list --all --limit 3 --output yaml
airs agentguard scans list --start 2026-08-09T00:00:00Z --end 2026-09-08T00:00:00Z --output table
airs agentguard scans vulnerabilities <scan-uuid> --output json
airs agentguard stats --time-period 30_DAYS --output json
airs agentguard rules list --all --limit 3 --output yaml
```

All reads support `pretty`, `table`, `markdown`, `csv`, `json`, and `yaml`. Scans and rules lists support `--offset` (default 0), `--limit` (default 10), `--all`, and `--max` (default 10000; 0 removes the item cap, but a 1000-page safety bound remains). JSON/YAML include pagination metadata; `truncated: true` means more records may remain. The `ls` alias works for list commands.

For rules, the server's `total_items` is a page count. The CLI walks until a short/empty page and reports `total_items: null` when the overall size is unknown. It does not silently truncate at the first page. Scans use the server's global total and reject repeated identities or changing pagination during `--all`.

Scan lists omit names, URLs, device metadata, fingerprints and evaluation descriptions. Finding commands omit paths, descriptions and source code unless **`--include-content`** is explicitly supplied. Treat that opt-in output as sensitive; it is not intended for public deliverables. Debug files remain private CWD artifacts and omit AgentGuard request/response bodies, including with endpoint overrides.

Only the captured/live-verified `30_DAYS` statistics period is supported. Unsupported periods and malformed queries fail; missing scan metrics are returned as null, not zero.

## Generate a report

```bash
airs agentguard report
airs agentguard report --strict --output markdown
airs agentguard report --strict --output-file ./agentguard-review.html
airs agentguard report --output markdown --output-file - > agentguard-review.md
```

HTML is the default regardless of terminal output preferences. The default destination is a new `airs-agentguard-report-<timestamp>-<random>.html` file in the current directory (or `.md` for Markdown). Files are mode 0600, created atomically without overwriting existing files or symlinks. `--output-file -` writes only the report to stdout; status messages use stderr. HTML is self-contained with inline CSS/JavaScript, no external assets, and supports offline filtering and printing.

`--max-pages` sets a per-inventory budget from 1 to 100 (default 40, 10 records per page). Incomplete sources are labeled and produce review findings. `--strict` exits 1 after writing if any source is incomplete; an entirely unavailable environment exits 1 even without strict mode. Security findings do not by themselves fail the command. `--debug` is rejected for report generation to protect deliverables. `--title` accepts 1–240 characters.

The report combines a bounded 30-day scan inventory, server rolling `30_DAYS` skill statistics, and the current rule catalog. It does not sum scan finding counts or count batch/parent rows alongside children. Rule defaults are not effective policy configuration, and independently read windows need not reconcile. The report contains aggregates, not finding text, names, IDs or source code.

## Actual live CLI output — September 8, 2026

These results came from CLI 5.4.0's release build with registry-installed SDK 0.29.0 and fresh OAuth authentication, not from replayed browser responses. The release acceptance rerun at 18:39 UTC passed all **26 checks**, with no local SDK link and no credential-file changes.

```bash
airs agentguard stats --output json
```

```json
{
  "unique_skills_scanned": {
    "count": 2,
    "percent_change": null
  },
  "total_vulnerabilities_found": {
    "count": 14,
    "percent_change": null
  },
  "top_vulnerability": {
    "vulnerability_type": "SECRET_EXPOSURE",
    "finding_count": 7
  }
}
```

Verified `scans list --all --limit 3` returned **24/24** historical scans; `rules list --all --limit 3` returned **7/7** rules, not just the first three. Finding metadata returned **6** findings for a discovered scan without revealing content.

The following table is an excerpt of the generated Markdown report, not a complete raw scan response:

| Artifact / outcome | Scan rows |
| --- | --- |
| AGENT / PENDING | 3 |
| SKILL / ALLOWED | 8 |
| SKILL / BLOCKED | 2 |

That report collected **13** scans across **2** pages within its 30-day window. All three sources were complete; both strict HTML and strict Markdown commands exited 0. Attention findings included **2 blocked scans** and **14 server-reported vulnerabilities**; **3 scans** lacked a completed decision. Null percentage changes rendered as “Unknown.” No credential file was modified.

## Reproduce local acceptance

After installing dependencies and building this CLI:

```bash
pnpm run build
AIRS_BROWSER_CAPTURE_PATH=/var/tmp/t3.txt node scripts/e2e-agentguard.mjs
```

The optional capture variable validates response contracts without executing curl or using captured tokens. Live checks use the configured credentials. The runner exercises all nine SDK reads, CLI formats, pagination, privacy, HTML/Markdown/stdout reports, no-overwrite behavior, Model Security filters and the unchanged credential-file hash. It writes private artifacts beneath `artifacts/agentguard-e2e-*` and prints only sanitized evidence. The first live run exposed null scan metrics and incorrect rule totals; regression tests and a passing rerun followed.
