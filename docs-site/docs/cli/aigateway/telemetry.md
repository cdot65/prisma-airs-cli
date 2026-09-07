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

SCM returns `null` mean/percentile aggregates for an empty latency window. CLI 4.2.2 pins SDK 0.23.0,
which preserves these valid nulls in JSON/YAML instead of rejecting the response. Null is not
measured zero latency; time buckets remain numeric. Earlier SDK 0.22.0 and older releases rejected
this response. Do not manufacture traffic merely to make an empty-window response validate.

SDK 0.23.0 also exposes trace/metadata filters on four chart methods, but this CLI release does not
add chart filter flags. `--trace-id` remains a `logs list` option, not a chart option.

## Verified empty-window output

The separately installed CLI 4.2.2 passed **3/3** read-only checks at **2026-09-07T07:39:51.294Z**, including actual JSON and YAML commands with SDK 0.23.0. No traffic or key was created to populate the empty window.

Actual installed CLI JSON/YAML output projected to exit status, period aggregate values and a zero-bucket check. No tenant identifiers, count aggregates or credentials are published. This historical empty-window check does not test new CLI filter flags.

```json
{
  "version": "4.2.2",
  "sdkVersion": "0.23.0",
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
