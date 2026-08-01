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
| `--workspace <slug>` | Yes | — | Workspace slug (e.g. `ws-main-a-349e0e`) |
| `--days <n>` | No | `7` | Rolling window in days, counted back from now |
| `--output <format>` | No | `pretty` | Output format: pretty, json, yaml |

:::warning Costs are in cents

The API reports every cost value in **cents** and never converts. Pretty
output shows dollars; `--output json|yaml` keeps the raw values in explicitly
named `totalCents` / `avgCents` / `costCents` fields.

:::

#### Examples

```bash
airs aigateway telemetry cost --workspace ws-main-a-349e0e
airs aigateway telemetry cost --workspace ws-main-a-349e0e --days 30 --output json
```

The other telemetry surfaces (requests, tokens, latency, group-bys, raw logs)
are not yet exposed by the CLI — scoped for a future release.
