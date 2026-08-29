---
sidebar_label: telemetry
---

# aigateway telemetry

Runtime telemetry for AI Gateway workspaces — the data behind the SCM
Observability tabs. Data plane; keyed by workspace **slug**, not UUID.

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
| `--output <format>` | No | `pretty` | Output format: pretty, table, markdown, csv, json, yaml |

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

The other telemetry surfaces (requests, tokens, latency, group-bys, raw logs)
are not yet exposed by the CLI — scoped for a future release.
