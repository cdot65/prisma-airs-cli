---
sidebar_label: resources and CRUD
sidebar_position: 2
---

# AI Gateway resources and CRUD

For task-oriented setup, workspace creation, and integration-binding examples, start with the
[AI Gateway workflow cheat sheet](workflows.md).

`airs aigateway` exposes the AI Gateway surface in SDK 0.20.0. Commands follow one grammar:

```text
airs aigateway <resource> <action> [id]
```

Run any group with `--help` to see its identifiers, required flags, and examples. All reads accept
`--output pretty|table|markdown|csv|json|yaml`; JSON list output is a bare array. AI Gateway reuses
the `PANW_MGMT_*` OAuth credentials and optionally accepts the `PANW_AI_GW_*_ENDPOINT` overrides.

## Command map

| Resource | Commands | Plane and notes |
| --- | --- | --- |
| `api-keys service` | `list`, `get`, `create`, `update`, `delete`, `rotate` | Data plane; list requires a workspace UUID. |
| `api-keys user` | `list`, `get`, `create`, `update`, `delete`, `rotate` | Data plane; user create bodies include `user_id`. |
| `audit-logs` | `list` | Admin plane; defaults to the previous seven days. |
| `configs` | `list`, `get`, `versions`, `create`, `update`, `delete` | Data plane; delete is permanent. |
| `deployments` | `list`, `get`, `create`, `update`, `archive`, `ping` | Admin plane; archive is soft removal. |
| `guardrails` | `list`, `get`, `create`, `update`, `delete` | Data plane; delete is permanent. |
| `integrations` | `list`, `get`, `create`, `update`, `delete` | Admin plane; includes `models list/set` and `workspaces list/set`. |
| `mcp integrations` | `list`, `get`, `create`, `update`, `delete` | Admin plane; includes capabilities, metadata, and workspace access. |
| `organisations` | `self get/update`, `auth-settings get/update` | Admin plane; auth settings require the numeric TSG id. |
| `plugins` | `list`, `create` | Admin plane; the SDK has no verified get/update/delete endpoints. |
| `providers` | `list`, `get`, `create`, `update`, `delete` | Data plane; detail credentials are redacted by default. |
| `telemetry` | cache, cost, errors, feedback, grouping, latency, logs, requests, retries, tokens, users | Data plane; uses workspace slug. |
| `workspaces` | `list`, `get`, `create`, `update`, `archive` | Reads span both planes; writes use the admin plane. |

`workspace` remains an accepted compatibility alias for `workspaces`. The deprecated
`workspace delete` spelling still archives and prints a warning; it deliberately has no `rm`
alias. Use `workspaces archive` in new automation.

## Structured mutation input

AI Gateway mutations use named flags for stable fields and repeatable dotted assignments for nested
configuration. No request file is required:

```bash
# Create a routing config
airs aigateway configs create \
  --name primary-routing \
  --workspace <workspace-uuid> \
  --set config.retry.attempts=3 \
  --set config.strategy.mode=fallback \
  --output json

# Create a guardrail one field at a time
airs aigateway guardrails create \
  --name deny-risk \
  --workspace <workspace-uuid> \
  --set 'checks[0].id=prompt-injection' \
  --set actions.deny=true

# Preserve existing MCP bindings while changing one workspace
airs aigateway mcp integrations workspaces set <integration-id> \
  --workspace-binding <workspace-id>=false \
  --global-access false \
  --preserve-existing \
  --force --output json
```

`--set <path=value>` parses JSON scalars, arrays, and objects when possible; otherwise the value is
a string. Use `--set-string <path=value>` when a value such as `123`, `true`, or `null` must remain
a literal string. Dot segments create objects and bracket indexes create arrays. Unsafe prototype
segments, sparse arrays, conflicting paths, non-finite values, and request-schema violations fail
before OAuth or network access.

The dotted path addresses the SDK request body, so config routing settings begin with `config.` and
integration-specific settings begin with `configurations.`. Run the exact leaf command with
`--help` for its named flags and known values sourced from SDK 0.20 catalogs.

### File escape hatch

`--file <json|yaml>` remains an optional advanced base for generated or provider-specific bodies.
Named flags override file fields, and `--set` / `--set-string` apply last:

```bash
airs aigateway integrations update <integration-id> \
  --file provider-base.yaml \
  --name vertex-production \
  --set configurations.vertex_region=us-central1
```

The merged body is validated against the exported SDK operation schema. Relationship `set`
commands replace access, models, or capabilities by default and therefore require confirmation;
pass `--preserve-existing` only when workspace bindings should be additive. Use `--force` only
after inspecting the exact target id and options.

Hard `delete` operations also require confirmation and receive the `rm` alias. Workspace and
deployment soft removal is named `archive`, never `rm`. Integration delete and deployment archive
also require `--organisation-id <numeric-tsg-id>`.

## One-time credentials

API-key create/rotate, deployment create, and deployment updates with `--rotate-auth true` refuse
to call the API until a secret destination is chosen:

```bash
airs aigateway api-keys service create \
  --name ci-gateway \
  --organisation-id <numeric-tsg-id> \
  --workspace <workspace-uuid> \
  --type workspace \
  --scopes completions.write \
  --secret-output ./api-key.secret.json

airs aigateway deployments create \
  --name private-gateway \
  --type production \
  --organisation-id <numeric-tsg-id> \
  --secret-output ./deployment.secret.json

airs aigateway deployments update <deployment-id> \
  --rotate-auth true \
  --secret-output ./rotated-deployment.secret.json
```

`--secret-output` reserves a new file with mode `0600` before confirmation or the API call and will
not overwrite an existing path. API failure or declined confirmation removes that reserved file.
`--show-secret` is available for deliberate piping. Debug API logs recursively redact tokens,
secrets, credentials, passwords, authorization fields, and API keys. API-key list/detail reads,
provider detail, and organisation authentication settings are redacted by default; their
`--reveal-sensitive` flags are explicit opt-ins. Operation-scoped secret paths come from SDK 0.20
metadata.

## Deployment health

`deployments get` reports the control plane's deployment record and heartbeat state.
`deployments ping` is a separate, optional control-plane-to-data-plane ingress diagnostic. A private
gateway can be healthy through outbound heartbeat while `ping` fails because ingress is intentionally
blocked; do not interpret the ping result as the only health signal.

## Local live-safe E2E

Place management credentials in the ignored, owner-only `.env.ai-gateway.local` file and run:

```bash
pnpm test:e2e:aigateway
```

Set `AI_GATEWAY_E2E_WORKSPACE_SLUG` to override the default dev workspace
`ws-develo-71f8d8`. Inventory and telemetry checks are read-only. One mutation test creates,
updates, verifies, and hard-deletes a uniquely named disposable config through structured flags;
its `finally` cleanup retries the exact delete if an intermediate assertion fails. The suite does
not alter established resources.
