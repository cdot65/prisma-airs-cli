---
sidebar_label: telemetry
---

# aigateway telemetry

Runtime telemetry for AI Gateway workspaces — the data behind the SCM
Observability tabs. Data plane; keyed by workspace **slug**, not UUID.

All telemetry reads accept `--workspace <slug>`, `--days <n>` (default `7`), optional explicit
`--start`/`--end` ISO-8601 timestamps, and the shared output formats.

| Command | Result |
| --- | --- |
| `cache summary` / `cache trend` | Cache totals and hit-rate series |
| `cost` | Spend totals and daily records |
| `errors` / `error-trends` | Error count and trends |
| `feedback distribution|models|trend|weighted` | Feedback analytics |
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
airs aigateway telemetry group-by model --workspace ws-main-a-349e0e --columns cost,tokens
airs aigateway telemetry logs list --workspace ws-main-a-349e0e --status-code 446 --output json
airs aigateway telemetry feedback distribution --workspace ws-main-a-349e0e --days 7
```

An entirely empty latency window currently returns valid `null` aggregate values from SCM that SDK
0.20.0 rejects during response validation. Choose a window containing traffic until the SDK schema
accepts nullable empty-window aggregates.
