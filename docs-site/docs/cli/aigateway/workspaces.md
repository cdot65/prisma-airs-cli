---
sidebar_label: workspace
---

# aigateway workspace

Manage **AI Gateway workspaces** — the unit almost every other AI Gateway
resource is keyed by. Credentials are shared with the management API
(`PANW_MGMT_*`); only the endpoints are AI Gateway-specific
(`PANW_AI_GW_DATA_ENDPOINT`, `PANW_AI_GW_ADMIN_ENDPOINT`, with
`PANW_AI_GW_TOKEN_ENDPOINT` falling back to `PANW_MGMT_TOKEN_ENDPOINT`).

:::warning Two planes, and the default hides rows

The AI Gateway spans two planes with **different SCM role scopes**:

- **data** (`/ai_gw/v2`) — returns only workspaces your service account holds a
  workspace-scope grant on. Needs `view_only_admin` or higher on the
  `main_airs_workspace_<TSG>` scope.
- **admin** (`/ai_gw/admin/v2`) — returns every workspace in the tenant. Needs
  an admin role at tenant-root scope.

A bare `list` is therefore **not** "all workspaces": it shows only *active*
workspaces *you are scoped to*. Use `--plane admin` for the whole tenant and
`--all` to also include archived rows.

On a 403 the CLI tells you which grant is missing: `errorCode AB03` means the
workspace-scope grant (data plane); otherwise the tenant-root admin grant. Note
SCM's Access Management UI *edits* the existing role row by default — click
**Add Role** so the account ends up with both role rows.

:::

### aigateway workspace list

List workspaces.

```text
airs aigateway workspace list [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--plane <plane>` | No | `data` | Plane to read from: `data` (scoped) or `admin` (whole tenant) |
| `--status <status>` | No | `active` | Filter by lifecycle state: `active` or `archived` |
| `--all` | No | — | Merge active + archived admin-plane reads (whole tenant, both states) |
| `--output <format>` | No | `pretty` | Output format: pretty, table, csv, json, yaml |

Without `--status`, archived workspaces are omitted entirely — there is no
single call returning both states, so `--all` merges two admin-plane reads.

#### Examples

```bash
airs aigateway workspace list
airs aigateway workspace list --plane admin
airs aigateway workspace list --plane admin --status archived
airs aigateway workspace list --all --output json
```

### aigateway workspace get

Get one workspace by UUID **or** slug, including the settings blocks list rows
do not carry.

```text
airs aigateway workspace get <ref> [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--plane <plane>` | No | `data` | Plane to read from: `data` (scoped) or `admin` (whole tenant) |
| `--output <format>` | No | `pretty` | Output format: pretty, json, yaml |

A workspace outside your workspace scope answers `403 AB03` on the data plane
(not 404) — re-read it with `--plane admin`. An **archived** workspace answers
`404 AB08` for both its UUID and slug on either plane; inspect archived rows
via `list --status archived` instead.

:::note Status can disagree between endpoints

`list` reports `active` for workspaces whose `get` reports `null`. The CLI
renders a null status as `unknown` — treat it as unknown, never as inactive,
and prefer the list value.

:::

#### Examples

```bash
airs aigateway workspace get ws-main-a-349e0e
airs aigateway workspace get 16f7e90d-382a-4e78-b577-1b01eb5f8297 --plane admin --output json
```
