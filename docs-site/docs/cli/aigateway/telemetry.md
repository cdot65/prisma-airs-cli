---
sidebar_label: telemetry
sidebar_position: 4
---

# aigateway telemetry

Runtime telemetry for AI Gateway workspaces — the data behind the SCM
Observability tabs. Data plane; keyed by workspace **slug**, not UUID.

Telemetry reads accept `--workspace <slug>`, `--days <n>` (default `7`), and shared output formats.
The chart/group/log commands also accept explicit `--start`/`--end` ISO-8601 timestamps;
the legacy `cost` command currently accepts only a rolling-day window, as listed below.

| Command | Result |
| --- | --- |
| `cache summary` / `cache trend` | Cache totals and hit-rate series |
| `cost` | Spend totals and daily records |
| `errors` / `error-trends` | Error count and trends |
| `feedback distribution` / `models` / `trend` / `weighted` | Feedback analytics |
| `group-by <dimension>` | Aggregation by an SDK-supported dimension; `--columns` adds aggregates |
| `latency` | Total and percentile latency series |
| `logs list` | Request logs; supports `--page-size`, `--status-code`, and `--trace-id` |
| `requests` | Request count series |
| `rescued-retries` | Requests recovered by retry behavior |
| `tokens` | Token usage series |
| `users` / `user-trends` | Unique-user count and trends |

### aigateway telemetry cost

Total and per-day spend for a workspace.

```text
airs aigateway telemetry cost --workspace <slug> [--days <n>] [--output <format>]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--workspace <ref>` | Yes | — | Workspace slug (e.g. `ws-main-a-349e0e`); UUID or display name also accepted (CLI resolves to the slug) |
| `--days <n>` | No | `7` | Rolling window in days, counted back from now |
| `--output <format>` | No | Resolved | Output format: pretty, table, markdown, csv, json, yaml |

:::warning Costs are in cents

The API reports every cost value in **cents**. Pretty output shows dollars;
structured output retains `totalCents`, `avgCents`, and `costCents` and adds
the explicit converted fields `totalUsd`, `avgUsd`, and per-record `costUsd`.
No consumer needs to infer a unit or silently rescale a value.

:::

#### Examples

```bash
airs aigateway telemetry cost --workspace ws-main-a-349e0e
airs aigateway telemetry cost --workspace ws-main-a-349e0e --days 30 --output json
```

Additional examples:

```bash
airs aigateway telemetry requests --workspace ws-main-a-349e0e --days 30 --output json
airs aigateway telemetry group-by model --workspace ws-main-a-349e0e --columns cost,total_tokens
airs aigateway telemetry logs list --workspace ws-main-a-349e0e --status-code 446 --output json
airs aigateway telemetry feedback distribution --workspace ws-main-a-349e0e --days 7
```

SCM returns `null` mean/percentile aggregates for an empty latency window. SDK 0.23.0 and later
(including CLI 4.3.0's pinned SDK 0.24.0) preserve these valid nulls in JSON/YAML. Null is not
measured zero latency; time buckets remain numeric. Earlier SDK 0.22.0 and older releases rejected
this response. Do not manufacture traffic merely to make an empty-window response validate.

CLI 4.3.0 pins SDK 0.24.0 and adds the following filters to `requests`, `cost`, `tokens` and `latency`.
Other charts and groups do not accept these flags. `logs list` retains its separate singular `--status-code` option.

## Chart filters

| Flag | Contract |
| --- | --- |
| `--trace-id <id>` | One nonblank trace ID |
| `--metadata <json>` | JSON object with string values; all pairs must match |
| `--status-codes <codes>` | Nonnegative integer status codes, comma-separated |
| `--api-key-ids <ids>` | Service key UUIDs, comma-separated; never secret key values |
| `--ai-org-models <pairs>` | Comma-separated `provider__model` pairs, e.g. `openai__gpt-5.6-terra` |
| `--total-units-min <n>` / `--total-units-max <n>` | Inclusive nonnegative integer total-token bounds |
| `--cost-min <cents>` / `--cost-max <cents>` | Inclusive finite nonnegative bounds in cents; fractional cents allowed |

Lists use OR within the list; different filters use AND. Equal minimum/maximum bounds and zero are preserved.
Empty CSV elements, malformed JSON/numbers, invalid UUIDs and reversed ranges exit 2 before client creation or workspace resolution.
The SDK supplies the shared semantic validation; the CLI only parses argument syntax. Analytics uses `provider__model`,
not the runtime inference routing syntax `@provider/model`.

```bash
airs aigateway telemetry requests --workspace ws-develo-71f8d8 --days 7 \
  --status-codes 200,446 --metadata '{"environment":"dev"}' --output json
airs aigateway telemetry cost --workspace dev --days 7 \
  --ai-org-models openai__gpt-5.6-terra --cost-min 0 --cost-max 0.125 --output json
airs aigateway telemetry tokens --workspace ws-develo-71f8d8 --days 7 \
  --total-units-min 0 --total-units-max 128 --output yaml
```

These syntax examples are not fabricated response captures. The read-only installed-CLI verification output below is recorded separately.

## Verified chart-filter output

The independently installed CLI **4.3.1**, with SDK **0.24.0**, passed **54/54** read-only checks at **2026-09-07T10:32:02.882Z**: installed-version verification, an owned positive control and 13 positive/empty-cohort pairs for each of four charts. Every pair runs the public CLI executable.

Existing owned positive traffic only. Inclusive bounds, singleton/CSV-OR alternatives and combined filters are checked on each chart with an empty negative cohort. No trace IDs, keys, metadata values or tenant counts are published. This does not establish full upstream analytics equivalence.

Cost uses its existing seven-day window and verifies explicit cents/USD conversion fields; requests, tokens and latency use the original fixture's exact window. Trace and metadata isolation select the same owned historical request. No inference, key or workspace is created by this suite.

Actual captured query-contract output (booleans and empty aggregates only):

<details>
<summary>All 52 positive/empty-cohort pairs</summary>

```json
[
  {
    "metric": "requests",
    "filter": "statusCodes.single",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": 0,
    "respected": true
  },
  {
    "metric": "requests",
    "filter": "statusCodes.csv-or",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": 0,
    "respected": true
  },
  {
    "metric": "requests",
    "filter": "apiKeyIds.single",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": 0,
    "respected": true
  },
  {
    "metric": "requests",
    "filter": "apiKeyIds.csv-or",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": 0,
    "respected": true
  },
  {
    "metric": "requests",
    "filter": "aiOrgModels.single",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": 0,
    "respected": true
  },
  {
    "metric": "requests",
    "filter": "aiOrgModels.csv-or",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": 0,
    "respected": true
  },
  {
    "metric": "requests",
    "filter": "totalUnitsMin.inclusive",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": 0,
    "respected": true
  },
  {
    "metric": "requests",
    "filter": "totalUnitsMax.inclusive",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": 0,
    "respected": true
  },
  {
    "metric": "requests",
    "filter": "costMin.inclusive",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": 0,
    "respected": true
  },
  {
    "metric": "requests",
    "filter": "costMax.inclusive",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": 0,
    "respected": true
  },
  {
    "metric": "requests",
    "filter": "totalUnits.exact-range",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": 0,
    "respected": true
  },
  {
    "metric": "requests",
    "filter": "cost.exact-range",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": 0,
    "respected": true
  },
  {
    "metric": "requests",
    "filter": "all.intersection",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": 0,
    "respected": true
  },
  {
    "metric": "cost",
    "filter": "statusCodes.single",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": 0,
    "respected": true
  },
  {
    "metric": "cost",
    "filter": "statusCodes.csv-or",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": 0,
    "respected": true
  },
  {
    "metric": "cost",
    "filter": "apiKeyIds.single",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": 0,
    "respected": true
  },
  {
    "metric": "cost",
    "filter": "apiKeyIds.csv-or",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": 0,
    "respected": true
  },
  {
    "metric": "cost",
    "filter": "aiOrgModels.single",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": 0,
    "respected": true
  },
  {
    "metric": "cost",
    "filter": "aiOrgModels.csv-or",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": 0,
    "respected": true
  },
  {
    "metric": "cost",
    "filter": "totalUnitsMin.inclusive",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": 0,
    "respected": true
  },
  {
    "metric": "cost",
    "filter": "totalUnitsMax.inclusive",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": 0,
    "respected": true
  },
  {
    "metric": "cost",
    "filter": "costMin.inclusive",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": 0,
    "respected": true
  },
  {
    "metric": "cost",
    "filter": "costMax.inclusive",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": 0,
    "respected": true
  },
  {
    "metric": "cost",
    "filter": "totalUnits.exact-range",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": 0,
    "respected": true
  },
  {
    "metric": "cost",
    "filter": "cost.exact-range",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": 0,
    "respected": true
  },
  {
    "metric": "cost",
    "filter": "all.intersection",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": 0,
    "respected": true
  },
  {
    "metric": "tokens",
    "filter": "statusCodes.single",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": 0,
    "respected": true
  },
  {
    "metric": "tokens",
    "filter": "statusCodes.csv-or",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": 0,
    "respected": true
  },
  {
    "metric": "tokens",
    "filter": "apiKeyIds.single",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": 0,
    "respected": true
  },
  {
    "metric": "tokens",
    "filter": "apiKeyIds.csv-or",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": 0,
    "respected": true
  },
  {
    "metric": "tokens",
    "filter": "aiOrgModels.single",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": 0,
    "respected": true
  },
  {
    "metric": "tokens",
    "filter": "aiOrgModels.csv-or",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": 0,
    "respected": true
  },
  {
    "metric": "tokens",
    "filter": "totalUnitsMin.inclusive",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": 0,
    "respected": true
  },
  {
    "metric": "tokens",
    "filter": "totalUnitsMax.inclusive",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": 0,
    "respected": true
  },
  {
    "metric": "tokens",
    "filter": "costMin.inclusive",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": 0,
    "respected": true
  },
  {
    "metric": "tokens",
    "filter": "costMax.inclusive",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": 0,
    "respected": true
  },
  {
    "metric": "tokens",
    "filter": "totalUnits.exact-range",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": 0,
    "respected": true
  },
  {
    "metric": "tokens",
    "filter": "cost.exact-range",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": 0,
    "respected": true
  },
  {
    "metric": "tokens",
    "filter": "all.intersection",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": 0,
    "respected": true
  },
  {
    "metric": "latency",
    "filter": "statusCodes.single",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": null,
    "respected": true
  },
  {
    "metric": "latency",
    "filter": "statusCodes.csv-or",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": null,
    "respected": true
  },
  {
    "metric": "latency",
    "filter": "apiKeyIds.single",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": null,
    "respected": true
  },
  {
    "metric": "latency",
    "filter": "apiKeyIds.csv-or",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": null,
    "respected": true
  },
  {
    "metric": "latency",
    "filter": "aiOrgModels.single",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": null,
    "respected": true
  },
  {
    "metric": "latency",
    "filter": "aiOrgModels.csv-or",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": null,
    "respected": true
  },
  {
    "metric": "latency",
    "filter": "totalUnitsMin.inclusive",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": null,
    "respected": true
  },
  {
    "metric": "latency",
    "filter": "totalUnitsMax.inclusive",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": null,
    "respected": true
  },
  {
    "metric": "latency",
    "filter": "costMin.inclusive",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": null,
    "respected": true
  },
  {
    "metric": "latency",
    "filter": "costMax.inclusive",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": null,
    "respected": true
  },
  {
    "metric": "latency",
    "filter": "totalUnits.exact-range",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": null,
    "respected": true
  },
  {
    "metric": "latency",
    "filter": "cost.exact-range",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": null,
    "respected": true
  },
  {
    "metric": "latency",
    "filter": "all.intersection",
    "knownPositive": true,
    "absentEmpty": true,
    "absentAggregate": null,
    "respected": true
  }
]
```

</details>

This certifies the listed filters on these four Prisma SCM chart adapters, not every upstream analytics option or public WAN access.

## Verified empty-window output

The separately installed CLI 4.3.1 passed **3/3** read-only checks at **2026-09-07T10:31:01.470Z**, including actual JSON and YAML commands with SDK 0.24.0. No traffic or key was created to populate the empty window.

Actual installed CLI JSON/YAML output projected to exit status, period aggregate values and a zero-bucket check. No tenant identifiers, count aggregates or credentials are published. This historical empty-window check does not test new CLI filter flags.

```json
{
  "version": "4.3.1",
  "sdkVersion": "0.24.0",
  "json": {
    "exitCode": 0,
    "total": null,
    "p50": null,
    "p90": null,
    "p99": null,
    "zeroValuedBuckets": true
  },
  "yaml": {
    "exitCode": 0,
    "total": null,
    "p50": null,
    "p90": null,
    "p99": null,
    "zeroValuedBuckets": true
  }
}
```

Reproduce the empty-window read with:

```bash
airs aigateway telemetry latency --workspace ws-develo-71f8d8 --start 2020-01-01T00:00:00Z --end 2020-01-01T00:00:01Z --output json
```

This uses the disclosed TLS-verified LAN path and unchanged read-only SCM credentials. It is not certification of every gateway operation or public WAN access.
