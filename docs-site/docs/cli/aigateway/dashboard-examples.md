---
sidebar_label: Verified report examples
sidebar_position: 3
---

# AI Gateway report — verified CLI output

Captured from the globally npm-installed **CLI 5.3.0**, using published **SDK 0.28.0**, on
**2026-09-08**. All **5/5 live E2E workflows passed**, including all five new atomic reads,
redaction, distinct transaction pages, HTML/Markdown delivery, strict truncation and no-clobber/debug guards.
The credential file was unchanged. No inference or configuration changes were made.

This is exact Markdown stdout, not reconstructed SDK output. The fixed telemetry window is
September 7, 00:00–24:00 UTC. It is separate from the SDK capture's narrower 174-row window.
All 25 sources completed; 1,431 unique transactions were collected over 29 pages.
Current configurations are labeled separately from daily telemetry.

```bash
airs --quiet aigateway report --workspace ws-develo-71f8d8 \
  --start 2026-09-07T00:00:00Z --end 2026-09-08T00:00:00Z \
  --strict --output markdown --output-file -
```

<details>
<summary>Complete actual Markdown stdout</summary>

```markdown
# AI Gateway environment report

**Prisma AIRS AI Gateway**

Assessment: **Attention required**

Collection: 2026-09-08T15:34:49.954Z → 2026-09-08T15:34:57.652Z

Telemetry window &#40;UTC&#41;: 2026-09-07T00:00:00.000Z → 2026-09-08T00:00:00.000Z

## What needs attention

### REVIEW: Success-status responses appear in error analytics

Evidence: The server classifies some HTTP 2xx responses as errors. Error analytics are not identical to non-2xx transaction counts.

Next step: Inspect the underlying application/provider error classifications before calculating an HTTP failure rate.

Source: telemetry.errorCategoryTrends

### ATTENTION: Rate limiting or server errors observed

Evidence: Error categories include HTTP 429 or 5xx responses in the selected window.

Next step: Review provider availability, quotas and retry policies; correlate with transaction logs.

Source: telemetry.errorCategoryTrends

## Telemetry metrics

Server field names are preserved. cost: cents; latency and avgCacheLatency: ms; errorPercent: percent. Feedback is not a request count.

| Source | Metric | Value |
| --- | --- | --- |
| requests | total | 1431 |
| errors | total | 169 |
| users | total | 3 |
| cost | total | 331.83136399999995 |
| cost | avg | 0.19529168960024038 |
| latency | total | 551.7568134171908 |
| latency | p50 | 629 |
| latency | p90 | 1112 |
| latency | p99 | 2103.3000000000084 |
| tokens | total | 4208173 |
| tokens | total&#95;request&#95;units | 4158157 |
| tokens | total&#95;response&#95;units | 50016 |
| cacheSummary | summary.cacheHits | 0 |
| cacheSummary | summary.avgCacheLatency | Unknown |
| cacheSummary | summary.totalRequests | 1264 |
| cacheSummary | summary.cacheSpeedup | 0 |
| cacheHitTrend | summary.totalCacheHits | 0 |
| cacheHitTrend | summary.hitRate | 0 |
| userTrends | summary.total | 1431 |
| userTrends | summary.unique | 3 |
| userTrends | summary.avg | 477 |
| errorTrends | summary.errorPercent | 11.809923130677847 |
| errorCategoryTrends | summary.totalErrors | 169 |
| errorCategoryTrends | summary.totalUniqueErrorCodes | 6 |
| groupedErrors | total | 169 |
| rescuedRetries | total | 0 |
| rescuedRetries | retryTotal | 0 |
| rescuedRetries | fallbackTotal | 0 |
| feedbackTrend | total | 0 |
| feedbackWeighted | total | Unknown |
| feedbackScoreDistribution | total | 0 |
| feedbackModels | Models with feedback | 0 |
| filterBoundaries | unique&#95;ai&#95;models count | 24 |
| filterBoundaries | unique&#95;status&#95;codes count | 6 |

## Error categories

Server-reported errors by HTTP status; 446 denotes an AIRS block.

| HTTP status | Errors |
| --- | --- |
| 200 | 2 |
| 401 | 3 |
| 404 | 5 |
| 500 | 74 |
| 400 | 14 |
| 446 | 71 |

## Error timeline

Server UTC buckets; counts by HTTP status. Not a count of unique violating sessions.

| UTC bucket | HTTP status | Errors |
| --- | --- | --- |
| 2026-09-07T00:00:00.000Z | 404 | 4 |
| 2026-09-07T03:00:00.000Z | 400 | 1 |
| 2026-09-07T03:00:00.000Z | 404 | 1 |
| 2026-09-07T04:00:00.000Z | 401 | 3 |
| 2026-09-07T04:00:00.000Z | 500 | 3 |
| 2026-09-07T04:00:00.000Z | 446 | 6 |
| 2026-09-07T05:00:00.000Z | 500 | 20 |
| 2026-09-07T05:00:00.000Z | 446 | 13 |
| 2026-09-07T06:00:00.000Z | 446 | 4 |
| 2026-09-07T06:00:00.000Z | 500 | 51 |
| 2026-09-07T07:00:00.000Z | 446 | 12 |
| 2026-09-07T08:00:00.000Z | 200 | 1 |
| 2026-09-07T08:00:00.000Z | 400 | 1 |
| 2026-09-07T09:00:00.000Z | 446 | 4 |
| 2026-09-07T09:00:00.000Z | 200 | 1 |
| 2026-09-07T09:00:00.000Z | 400 | 1 |
| 2026-09-07T11:00:00.000Z | 446 | 11 |
| 2026-09-07T11:00:00.000Z | 400 | 11 |
| 2026-09-07T12:00:00.000Z | 446 | 5 |
| 2026-09-07T13:00:00.000Z | 446 | 12 |
| 2026-09-07T14:00:00.000Z | 446 | 4 |

## Current configuration and capabilities

Aggregate counts only. Active tenant workspaces are tenant-wide; keys and configs use the selected workspace.

| Source | Metric | Value |
| --- | --- | --- |
| workspaces.list | Active workspaces returned | 8 |
| configs.list | Rows returned | 3 |
| apiKeys.listService | Rows returned | 5 |
| apiKeys.listUser | Rows returned | 0 |
| organisations.getInfo | Configured limit categories | 8 |
| guardrails.getCatalog | Available evaluators | 83 |

## Transaction coverage

Unique collected transactions by HTTP status; only complete pagination represents the entire window. Raw rows are excluded.

| Metric | Value |
| --- | --- |
| Server total | 1431 |
| Unique collected | 1431 |
| HTTP 200 | 1264 |
| HTTP 400 | 14 |
| HTTP 401 | 3 |
| HTTP 404 | 5 |
| HTTP 446 | 71 |
| HTTP 500 | 74 |

## Evidence and completeness

Complete means the source was collected within its budget, not that the environment is secure. Unavailable is not zero.

| Source | SDK method | Window | Status | Records | Pages | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| workspaces.list | workspaces.list | Current active tenant workspaces &#40;admin&#41; | complete | 8 | 1 | Collection completed. |
| telemetry.requests | telemetry.requests | Displayed telemetry window | complete | 24 | 1 | Collection completed. |
| telemetry.errors | telemetry.errors | Displayed telemetry window | complete | 24 | 1 | Collection completed. |
| telemetry.users | telemetry.users | Displayed telemetry window | complete | 24 | 1 | Collection completed. |
| telemetry.cost | telemetry.cost | Displayed telemetry window | complete | 24 | 1 | Collection completed. |
| telemetry.latency | telemetry.latency | Displayed telemetry window | complete | 24 | 1 | Collection completed. |
| telemetry.tokens | telemetry.tokens | Displayed telemetry window | complete | 24 | 1 | Collection completed. |
| telemetry.cacheSummary | telemetry.cacheSummary | Displayed telemetry window | complete | 1 | 1 | Collection completed. |
| telemetry.cacheHitTrend | telemetry.cacheHitTrend | Displayed telemetry window | complete | 24 | 1 | Collection completed. |
| telemetry.userTrends | telemetry.userTrends | Displayed telemetry window | complete | 24 | 1 | Collection completed. |
| telemetry.errorTrends | telemetry.errorTrends | Displayed telemetry window | complete | 24 | 1 | Collection completed. |
| telemetry.errorCategoryTrends | telemetry.errorCategoryTrends | Displayed telemetry window | complete | 6 | 1 | Collection completed. |
| telemetry.groupedErrors | telemetry.groupedErrors | Displayed telemetry window | complete | 24 | 1 | Collection completed. |
| telemetry.rescuedRetries | telemetry.rescuedRetries | Displayed telemetry window | complete | 24 | 1 | Collection completed. |
| telemetry.feedbackTrend | telemetry.feedbackTrend | Displayed telemetry window | complete | 24 | 1 | Collection completed. |
| telemetry.feedbackWeighted | telemetry.feedbackWeighted | Displayed telemetry window | complete | 0 | 1 | Collection completed. |
| telemetry.feedbackScoreDistribution | telemetry.feedbackScoreDistribution | Displayed telemetry window | complete | 0 | 1 | Collection completed. |
| telemetry.feedbackModels | telemetry.feedbackModels | Displayed telemetry window | complete | 0 | 1 | Collection completed. |
| telemetry.filterBoundaries | telemetry.filterBoundaries | Displayed telemetry window | complete | 1 | 1 | Collection completed. |
| telemetry.logs | telemetry.logs | Displayed telemetry window | complete | 1431 | 29 | Collection completed. |
| configs.list | configs.list | Current selected workspace configuration | complete | 3 | 1 | Collection completed. |
| apiKeys.listService | apiKeys.listService | Current selected workspace configuration | complete | 5 | 1 | Collection completed. |
| apiKeys.listUser | apiKeys.listUser | Current selected workspace configuration | complete | 0 | 1 | Collection completed. |
| organisations.getInfo | organisations.getInfo | Current tenant configuration | complete | 1 | 1 | Collection completed. |
| guardrails.getCatalog | guardrails.getCatalog | Current tenant capability catalog &#40;not enabled guardrails&#41; | complete | 83 | 1 | Collection completed. |

## Scope and limitations

- Read-only operational evidence, not a security certification or availability SLA. No inference or configuration changes are performed.
- All telemetry uses the displayed fixed window. Configuration and organisation/catalog reads describe current state, not daily changes.
- Requests, errors, feedback and log totals are independent server aggregates, not an atomic snapshot. An AIRS block is not necessarily an outage.
- Cost values are cents; latency values are milliseconds. Feedback scores may be negative. Null means unknown, not zero.
- Only aggregate numbers and HTTP status codes are retained. Keys, user identities, workspace names/IDs, metadata, URLs, prompts, raw logs and configuration bodies are excluded.
- Catalog entries describe available capabilities, not enabled protection. Organisation limit sentinel values are not interpreted as quota exhaustion.
- Incomplete sources and page budgets are explicit. Current inventory lists have no verified continuation contract; total/has&#95;more mismatches are partial.

Prisma AIRS CLI · AI Gateway report schema 1
```

</details>

## Reproduce and interpret

Install with `npm install --global @cdot65/prisma-airs-cli@5.3.0` to run these commands.
Omit `--output` and `--output-file` to generate a new private HTML file in your current directory.
The HTML and Markdown formats use the same allowlisted report projection.

`--strict --max-pages 1` produced a partial deliverable and exit 1. Existing-file attempts
left the original byte-for-byte unchanged; `--debug` exited 2 before creating any debug file.
The five new atomic reads returned valid JSON through the CLI. Organisation settings and
analytics filter data were redacted by default, and log pages 0 and 1 had distinct IDs.

The report's attention finding is real environment evidence, not a failing CLI test. The server
reported 74 HTTP 500 errors and 71 AIRS blocks in this historical window. It also classified two
HTTP 200 responses as errors; the report flags that discrepancy without modifying server totals.
These observations do not establish current downtime or a security breach.

See [command options, privacy and limitations](./dashboard.md).
