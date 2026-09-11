---
sidebar_label: workspaces
sidebar_position: 3
---

# aigateway workspaces

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

`workspace` is retained as a compatibility alias, but new scripts should use the canonical plural
`workspaces` group.

### aigateway workspaces list

List workspaces.

```text
airs aigateway workspaces list [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--plane <plane>` | No | `data` | Plane to read from: `data` (scoped) or `admin` (whole tenant) |
| `--status <status>` | No | `active` | Filter by lifecycle state: `active` or `archived` |
| `--all` | No | — | Merge active + archived admin-plane reads (whole tenant, both states) |
| `--output <format>` | No | Resolved | Output format: pretty, table, markdown, csv, json, yaml |

Without `--status`, archived workspaces are omitted entirely — there is no
single call returning both states, so `--all` merges two admin-plane reads.

#### Examples

```bash
airs aigateway workspaces list
airs aigateway workspaces list --plane admin
airs aigateway workspaces list --plane admin --status archived
airs aigateway workspaces list --all --output json
```

### aigateway workspaces get

Get one workspace by UUID, slug, **or display name**, including the settings
blocks list rows do not carry. (The API itself accepts only UUID/slug; the CLI
resolves display names against the workspace list — an ambiguous name errors
with the matching slugs.)

```text
airs aigateway workspaces get <ref> [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--plane <plane>` | No | `data` | Plane to read from: `data` (scoped) or `admin` (whole tenant) |
| `--output <format>` | No | Resolved | Output format: pretty, table, markdown, csv, json, yaml |

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
airs aigateway workspaces get ws-main-a-349e0e
airs aigateway workspaces get 16f7e90d-382a-4e78-b577-1b01eb5f8297 --plane admin --output json
```

### aigateway workspaces create

Create a workspace. **Admin plane** — needs a tenant-root admin role.

```text
airs aigateway workspaces create --name <name> [--scope-name <scope>] [--existing-scope] [options]
```

:::info Three API calls, in SCM's own order

A workspace's `scope_name` is not a label: it names an **SCM IAM scope** that must exist before
the workspace does. This command runs the same sequence Strata Cloud Manager's UI runs (captured
2026-09-11):

1. `POST /iam/v1/scopes` — create the scope (`--scope-name`, or a generated `ws_<name>_<suffix>`).
2. `POST /ai_gw/admin/v2/workspaces` — create the workspace with that `scope_name`; the response
   carries the server-generated slug.
3. `PUT /iam/v1/scopes/<scope>` — bind the scope to the workspace slug. This is what actually
   grants data-plane access to the new workspace.

A bare create against a scope that does not exist yet fails with `400 AB01` — which is what the
September 6 verification hit. Use `--existing-scope` to skip step 1 and bind a scope you already
created (its other bindings are preserved).

:::

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--name <name>` | Yes | — | Display name |
| `--scope-name <scope>` | No | `ws_<name>_<suffix>` | IAM scope to create and bind, e.g. `ws_production_bx7qw0` |
| `--existing-scope` | No | — | Bind an IAM scope that already exists instead of creating one (requires `--scope-name`) |
| `--description <text>` | No | — | Workspace description (also used as the new scope's description) |
| `--icon <icon>` | No | — | Workspace icon |
| `--metadata <json>` | No | — | Sugar for `defaults.metadata` (flat string map) |
| `--defaults <json>` | No | — | Workspace defaults object |
| `--users <ids>` | No | — | Comma-separated user ids to seed the workspace with |
| `--usage-limits <json>` | No | — | Usage-limit policies — a JSON **array** of policy objects |
| `--rate-limits <json>` | No | — | Rate-limit policies — a JSON **array** of policy objects |
| `--output <format>` | No | `pretty` | Output format: pretty, json, yaml |

The workspace is rendered from a follow-up `get`, not from the write response (which omits
`status`, `is_default`, `icon`, both limit fields, and the settings blocks). A status line on
stderr names the scope and the slug it was bound to. Granting that scope to the service accounts
that should reach the workspace is still an SCM Access Management step.

Partial failures are reported, never hidden. If the workspace step fails after the scope was
created, the scope is deleted again and the error says whether that rollback worked. If the bind
step fails, the workspace exists but is unbound; the error names the slug and scope so
`airs aigateway scopes bind <scope> --workspace <slug>` can finish the job.

#### Examples

```bash
airs aigateway workspaces create --name truffles --description 'Online recipe generation application'
airs aigateway workspaces create --name Production --scope-name ws_production_bx7qw0
airs aigateway workspaces create --name Staging --scope-name ws_staging_q1x8mz --existing-scope
airs aigateway workspaces create --name Production \
  --metadata '{"env":"production"}' \
  --rate-limits '[{"type":"requests","unit":"rpm","value":100}]'
```

### aigateway workspaces update

Partial update — send only what changes. **Admin plane.** `<ref>` accepts
UUID, slug, or display name (a raw name sent to the API yields a misleading
`400 AB01 "No update fields provided"` — the CLI resolves it for you).

```text
airs aigateway workspaces update <ref> [options]
```

Takes the same writable flags as `create` (minus `--scope-name`, plus no
required flags); at least one must be given. The API acknowledges the write
with an empty body, so the CLI re-reads the workspace and renders that.

#### Examples

```bash
airs aigateway workspaces update ws-produc-985697 --description 'Production workloads, us-east'
airs aigateway workspaces update ws-produc-985697 --rate-limits '[{"type":"requests","unit":"rpm","value":50}]'
```

### aigateway workspaces archive

Archive a workspace. **Admin plane.** This intentionally has no `rm` alias because it is not a hard
delete.

```text
airs aigateway workspaces archive <ref> [--force]
```

:::warning archive does not destroy

There is **no hard delete** for workspaces. The row disappears from a default
`list` but remains under `list --plane admin --status archived`. After the
archive, `get` answers `404 AB08` for both the UUID and the slug on either plane
— that is expected, not an error.

:::

Prompts for confirmation unless `--force`; non-TTY runs require `--force`.

#### Examples

```bash
airs aigateway workspaces archive ws-produc-985697
airs aigateway workspaces archive ws-produc-985697 --force
```

The deprecated `airs aigateway workspace delete <ref>` compatibility spelling performs the same
archive, prints a warning, and deliberately does not receive the `rm` alias.

## aigateway scopes

Manage the **SCM IAM scopes** (`/iam/v1/scopes`) that a workspace's `scope_name` points at.
`workspaces create` runs the whole provisioning sequence; these commands expose each step on its
own, plus tenant-wide inspection. Same credentials and tenant-root admin role as the admin plane;
`PANW_IAM_ENDPOINT` (config `iamEndpoint`) overrides the base URL.

| Command | Description |
|---------|-------------|
| `scopes list` | Every scope in the tenant. A scope with no bound resources is **unbound** — usually the leftover of a provisioning run that failed between steps 1 and 2. |
| `scopes get <name>` | One scope by name. `id` is `<name>:<tsg>` and is not accepted as a key. |
| `scopes create --name <name> [--description <text>]` | Step 1 on its own: an unbound scope. |
| `scopes bind <name> --workspace <ref>` | Step 3 on its own. `<ref>` is a slug, UUID, or unique display name; SCM binds by slug, so the CLI resolves it. Existing bindings are kept and re-running is safe. |
| `scopes delete <name> [--force]` (alias `rm`) | Delete a scope. **Not live-verified** — SCM's UI was never observed deleting one; treat a `404`/`405` as the API declining. Prompts unless `--force`. |

`list`/`get` were verified live on 2026-09-11; `create` and `bind` send the exact bodies captured
from SCM's own workspace-creation flow.

#### Examples

```bash
airs aigateway scopes list --output json | jq '.[] | select(.resources == "")'   # unbound scopes
airs aigateway scopes get ws_production_bx7qw0
airs aigateway scopes create --name ws_production_bx7qw0 --description 'All production applications'
airs aigateway scopes bind ws_production_bx7qw0 --workspace ws-produc-985697
airs aigateway scopes delete ws_truffles_ggolfu --force
```
