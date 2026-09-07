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
CLI 4.4.0 pins SDK 0.25.0 and also exposes these filters on all six `group-by` dimensions. Other charts do not accept these flags. `logs list` retains its separate singular `--status-code` option.

## Grouped analytics

`group-by` accepts `ai_service`, `model`, `api_key`, `provider`, `status_code` and `users`.
All six accept the filters below, explicit start/end timestamps and structured output. Dimensions,
columns and filters are validated before client creation. The SDK remains the source of supported
column names and filter semantics; malformed arguments exit 2 without authentication or a request.

```bash
airs aigateway telemetry group-by model --workspace ws-develo-71f8d8 --days 7 \
  --status-codes 200,446 --cost-max 0.125 --columns cost,total_tokens --output json
airs aigateway telemetry group-by status_code --workspace ws-develo-71f8d8 --days 7 \
  --ai-org-models openai__gpt-5.6-terra --output yaml
airs aigateway telemetry group-by users --workspace ws-develo-71f8d8 --days 7 \
  --metadata '{"environment":"dev"}' --output json
```

Non-user dimensions support `--columns cost,avg_latency,avg_tokens,total_tokens,success_rate,last_seen`.
User grouping does **not** support columns and keeps the SDK's `{success,data:{records,...}}` envelope;
other groups keep `{object,data:[...],...}`. JSON/YAML preserve those envelopes, numeric values and
costs in cents. `group-by users` is distinct from the existing unique-user count command `users`.

These are observed SCM adapters, not full equivalence to all supplied upstream analytics operations.
Provider `traceId` is a verified SCM extension omitted from the pinned upstream provider schema.

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

## Verified grouped-filter output

The independently installed CLI **4.4.0**, with SDK **0.25.0**, passes **103/103** read-only checks at **2026-09-07T12:32:16.444Z**: executable-version verification, an owned positive control and all 101 positive/absent filter pairs across six groups. Each pair executes the public CLI twice; five non-user checks also verify exact cost/token columns. JSON/YAML envelope preservation additionally has 95 public-command regression tests.

Read-only discovery on existing owned traffic. Every filter requires both a known positive and an empty absent cohort. No tenant identifiers, trace IDs, key IDs, metadata values or tenant counts are published. This is not full upstream analytics equivalence.

Actual captured results (all 101 pairs, no tenant counts or identifiers):

<details>
<summary>All 101 grouped-filter pairs</summary>

```json
[
  {
    "dimension": "ai_service",
    "filter": "traceId",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "ai_service",
    "filter": "metadata",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "ai_service",
    "filter": "trace-and-metadata.intersection",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "ai_service",
    "filter": "statusCode.single",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "ai_service",
    "filter": "statusCode.csv-or",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "ai_service",
    "filter": "apiKeyIds.single",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "ai_service",
    "filter": "apiKeyIds.csv-or",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "ai_service",
    "filter": "aiOrgModel.single",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "ai_service",
    "filter": "aiOrgModel.csv-or",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "ai_service",
    "filter": "totalUnitsMin.inclusive",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "ai_service",
    "filter": "totalUnitsMax.inclusive",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "ai_service",
    "filter": "costMin.inclusive",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "ai_service",
    "filter": "costMax.inclusive",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "ai_service",
    "filter": "totalUnits.exact-range",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "ai_service",
    "filter": "cost.exact-range",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "ai_service",
    "filter": "all.intersection",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "ai_service",
    "filter": "all.with-columns",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "model",
    "filter": "traceId",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "model",
    "filter": "metadata",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "model",
    "filter": "trace-and-metadata.intersection",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "model",
    "filter": "statusCode.single",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "model",
    "filter": "statusCode.csv-or",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "model",
    "filter": "apiKeyIds.single",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "model",
    "filter": "apiKeyIds.csv-or",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "model",
    "filter": "aiOrgModel.single",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "model",
    "filter": "aiOrgModel.csv-or",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "model",
    "filter": "totalUnitsMin.inclusive",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "model",
    "filter": "totalUnitsMax.inclusive",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "model",
    "filter": "costMin.inclusive",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "model",
    "filter": "costMax.inclusive",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "model",
    "filter": "totalUnits.exact-range",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "model",
    "filter": "cost.exact-range",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "model",
    "filter": "all.intersection",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "model",
    "filter": "all.with-columns",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "api_key",
    "filter": "traceId",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "api_key",
    "filter": "metadata",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "api_key",
    "filter": "trace-and-metadata.intersection",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "api_key",
    "filter": "statusCode.single",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "api_key",
    "filter": "statusCode.csv-or",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "api_key",
    "filter": "apiKeyIds.single",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "api_key",
    "filter": "apiKeyIds.csv-or",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "api_key",
    "filter": "aiOrgModel.single",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "api_key",
    "filter": "aiOrgModel.csv-or",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "api_key",
    "filter": "totalUnitsMin.inclusive",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "api_key",
    "filter": "totalUnitsMax.inclusive",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "api_key",
    "filter": "costMin.inclusive",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "api_key",
    "filter": "costMax.inclusive",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "api_key",
    "filter": "totalUnits.exact-range",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "api_key",
    "filter": "cost.exact-range",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "api_key",
    "filter": "all.intersection",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "api_key",
    "filter": "all.with-columns",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "provider",
    "filter": "traceId",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "provider",
    "filter": "metadata",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "provider",
    "filter": "trace-and-metadata.intersection",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "provider",
    "filter": "statusCode.single",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "provider",
    "filter": "statusCode.csv-or",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "provider",
    "filter": "apiKeyIds.single",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "provider",
    "filter": "apiKeyIds.csv-or",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "provider",
    "filter": "aiOrgModel.single",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "provider",
    "filter": "aiOrgModel.csv-or",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "provider",
    "filter": "totalUnitsMin.inclusive",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "provider",
    "filter": "totalUnitsMax.inclusive",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "provider",
    "filter": "costMin.inclusive",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "provider",
    "filter": "costMax.inclusive",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "provider",
    "filter": "totalUnits.exact-range",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "provider",
    "filter": "cost.exact-range",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "provider",
    "filter": "all.intersection",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "provider",
    "filter": "all.with-columns",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "status_code",
    "filter": "traceId",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "status_code",
    "filter": "metadata",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "status_code",
    "filter": "trace-and-metadata.intersection",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "status_code",
    "filter": "statusCode.single",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "status_code",
    "filter": "statusCode.csv-or",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "status_code",
    "filter": "apiKeyIds.single",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "status_code",
    "filter": "apiKeyIds.csv-or",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "status_code",
    "filter": "aiOrgModel.single",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "status_code",
    "filter": "aiOrgModel.csv-or",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "status_code",
    "filter": "totalUnitsMin.inclusive",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "status_code",
    "filter": "totalUnitsMax.inclusive",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "status_code",
    "filter": "costMin.inclusive",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "status_code",
    "filter": "costMax.inclusive",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "status_code",
    "filter": "totalUnits.exact-range",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "status_code",
    "filter": "cost.exact-range",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "status_code",
    "filter": "all.intersection",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "status_code",
    "filter": "all.with-columns",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "users",
    "filter": "traceId",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "users",
    "filter": "metadata",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "users",
    "filter": "trace-and-metadata.intersection",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "users",
    "filter": "statusCode.single",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "users",
    "filter": "statusCode.csv-or",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "users",
    "filter": "apiKeyIds.single",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "users",
    "filter": "apiKeyIds.csv-or",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "users",
    "filter": "aiOrgModel.single",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "users",
    "filter": "aiOrgModel.csv-or",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "users",
    "filter": "totalUnitsMin.inclusive",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "users",
    "filter": "totalUnitsMax.inclusive",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "users",
    "filter": "costMin.inclusive",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "users",
    "filter": "costMax.inclusive",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "users",
    "filter": "totalUnits.exact-range",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "users",
    "filter": "cost.exact-range",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  },
  {
    "dimension": "users",
    "filter": "all.intersection",
    "knownPositive": true,
    "absentEmpty": true,
    "respected": true
  }
]
```

</details>

The earlier candidate run passed 102/103, with a provider/status-code query failure; it remains in the private history. This complete subsequent run passes without changing retries, validation, filters, credentials or infrastructure. It is not a claim of uninterrupted service availability. Direct Gateway coverage remains **138/242 (57.02%)**, and all 22 upstream analytics adaptations remain partial.

## Verified chart-filter output

The independently installed CLI **4.4.0**, with SDK **0.25.0**, passed **54/54** read-only checks at **2026-09-07T12:35:21.598Z**: installed-version verification, an owned positive control and 13 positive/empty-cohort pairs for each of four charts. Every pair runs the public CLI executable.

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

The separately installed CLI 4.4.0 passed **3/3** read-only checks at **2026-09-07T12:29:09.610Z**, including actual JSON and YAML commands with SDK 0.25.0. No traffic or key was created to populate the empty window.

Actual installed CLI JSON/YAML output projected to exit status, period aggregate values and a zero-bucket check. No tenant identifiers, count aggregates or credentials are published. This historical empty-window check does not test new CLI filter flags.

```json
{
  "version": "4.4.0",
  "sdkVersion": "0.25.0",
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
