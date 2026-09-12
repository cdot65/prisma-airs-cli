---
sidebar_label: workflow cheat sheet
sidebar_position: 1
---

# AI Gateway workflow cheat sheet

Use this page for copy-and-adjust AI Gateway workflows. For the complete command inventory and
every available flag, see [resources and CRUD](resources.md).

## Know the identifiers

AI Gateway commands use several identifiers for different jobs:

| Value | Meaning | Where it is used |
| --- | --- | --- |
| Workspace UUID | Immutable workspace record identifier | Integration, provider, config, guardrail, and API-key bindings |
| Workspace slug | Server-generated URL-safe workspace reference | Telemetry and workspace `get` commands |
| Workspace `scopeName` | SCM role scope controlling data-plane visibility | SCM Access Management; supplied as `--scope-name` during creation |
| Integration UUID | Organisation integration record identifier | Integration model and workspace relationship commands |

Do not substitute a workspace slug or `scopeName` where a relationship command asks for a
workspace UUID.

## Authenticate once

AI Gateway **management** reuses the same SCM OAuth client ID, client secret, and TSG ID as the management, Red
Team, and Model Security APIs. It does not require a second AI Gateway credential set:

```bash
airs tenant create dev      # prompts for the client ID, secret, and TSG ID
airs tenant switch dev
airs doctor
```

`aiGwDataEndpoint` and `aiGwAdminEndpoint` are optional base-URL overrides in the tenant file.
They are normally unset. They are not the URL of a privately deployed gateway. There is no
AI Gateway token endpoint: every product authenticates through `mgmtTokenEndpoint`.

[Runtime inference](inference.md) is a separate credential plane: it uses the deployed gateway
URL and `PANW_AI_GW_INFERENCE_API_KEY`, not management OAuth.

::::tip[Configuration-file alternative]

The corresponding config keys are `mgmtClientId`, `mgmtClientSecret`, and `mgmtTsgId`. Environment
variables override values in `~/.prisma-airs/config.json`.

::::

## Discover workspaces and integrations

List every active workspace in the tenant through the admin plane, then list integrations:

```bash
airs aigateway workspaces list --plane admin --output json |
  jq '.[] | {id, name, slug, scopeName}'

airs aigateway integrations list --output json |
  jq '.[] | {id, name, slug}'
```

Use `airs aigateway workspaces list --all --output json` when archived workspaces must also be
included. A bare workspace list uses the data plane and only returns active workspaces visible to
the caller's SCM workspace scope.

## Create a workspace

A workspace's `scope_name` names an **SCM IAM scope** that has to exist first. `workspaces create`
runs the three calls Strata Cloud Manager's UI runs (captured 2026-09-11): create the IAM scope,
create the workspace with that `scope_name`, then `PUT` the scope back with the new workspace slug
bound as a resource. That last step is what grants data-plane access. Skipping the first step is
why a bare create with a made-up scope returned HTTP 400 (AB01) on September 6.

Choose a display name; the server generates the UUID and slug, and the CLI generates a scope name
in SCM's own `ws_<name>_<suffix>` style unless `--scope-name` says otherwise:

```bash
airs aigateway workspaces create \
  --name Development \
  --description 'Development AI Gateway traffic' \
  --output json
```

To pick the scope name yourself, or to bind a scope created earlier with
`airs aigateway scopes create`, pass `--scope-name` (and `--existing-scope` for the latter):

```bash
airs aigateway workspaces create --name Development --scope-name ws_development_4k2p9x
airs aigateway workspaces create --name Development --scope-name ws_development_4k2p9x --existing-scope
```

Confirm the resulting values and the binding:

```bash
airs aigateway workspaces list --plane admin --output json |
  jq '.[] | select(.name == "Development") | {id, name, slug, scopeName}'

airs aigateway scopes get ws_development_4k2p9x --output json |
  jq '.resources'      # => [{ "resourceType": "workspace", "resourceId": "<slug>" }]
```

The workspace's `scopeName` must also be granted to the intended service account through SCM
Access Management. If no caller holds that workspace-scope role, the workspace remains visible on
the admin plane but will not appear in a normal data-plane list. If provisioning stops after the
workspace was created, the error names the slug and scope; finish with
`airs aigateway scopes bind <scope> --workspace <slug>`.

## Add an integration to a workspace

This is the CLI equivalent of opening an integration in the GUI and adding a workspace. First
inspect its current bindings:

```bash
airs aigateway integrations workspaces list <integration-id> --output json
```

Then enable the workspace while preserving every binding not mentioned by this command:

```bash
airs aigateway integrations workspaces set <integration-id> \
  --workspace-binding <workspace-uuid>=true \
  --preserve-existing
```

For non-interactive automation, add `--force` only after confirming both UUIDs. Verify the result:

```bash
airs aigateway integrations workspaces list <integration-id> --output json
```

### Remove one workspace without disturbing others

```bash
airs aigateway integrations workspaces set <integration-id> \
  --workspace-binding <workspace-uuid>=false \
  --preserve-existing
```

### Replace the complete binding set

Without `--preserve-existing`, the command replaces all existing workspace bindings. Name every
workspace that should remain enabled:

```bash
airs aigateway integrations workspaces set <integration-id> \
  --global-access false \
  --workspace-binding <workspace-a-uuid>=true \
  --workspace-binding <workspace-b-uuid>=true
```

### Grant the integration global workspace access

```bash
airs aigateway integrations workspaces set <integration-id> \
  --global-access true
```

For an integration that should create a provider binding as it is attached, add:

```text
--create-default-provider true --default-provider-slug <provider-slug>
```

## Enable integration models

Inspect the available model slugs before changing model bindings:

```bash
airs aigateway integrations models list <integration-id> --output json
```

The model `set` command represents the desired binding set, so include every model that should
remain configured:

```bash
airs aigateway integrations models set <integration-id> \
  --allow-all-models false \
  --model <model-a-slug>=true \
  --model <model-b-slug>=true
```

## Bind an MCP integration

MCP workspace access follows the same additive-versus-replacement rule:

```bash
# Add one workspace and preserve the rest
airs aigateway mcp integrations workspaces set <mcp-integration-id> \
  --workspace-binding <workspace-uuid>=true \
  --preserve-existing

# Verify workspace access from integration detail
airs aigateway mcp integrations workspaces list <mcp-integration-id> --output json
```

To disable one MCP workspace binding, send the same binding with `=false` and keep
`--preserve-existing`.

## Query workspace telemetry

Relationship commands use the workspace UUID, but telemetry uses the workspace slug:

```bash
airs aigateway telemetry requests --workspace <workspace-slug> --days 7
airs aigateway telemetry cost --workspace <workspace-slug> --days 30 --output json
airs aigateway telemetry logs list --workspace <workspace-slug> --page-size 50
```

See the [telemetry reference](telemetry.md) for every metric and filter.

## Troubleshooting

### `AISEC_OAUTH_ERROR: Error running access token modification plugin`

This happens during the shared SCM token exchange, before an AI Gateway resource request is sent.
Confirm that the client ID, client secret, and TSG ID belong to the same SCM service account and
remove stale endpoint overrides:

```bash
airs tenant unset <name> mgmtTokenEndpoint
airs tenant unset <name> aiGwDataEndpoint
airs tenant unset <name> aiGwAdminEndpoint
airs doctor
```

### Workspace appears on the admin plane but not the data plane

The OAuth identity lacks the SCM workspace role for that workspace's `scopeName`. Read the value
from the admin plane and add the corresponding role scope in SCM Access Management:

```bash
airs aigateway workspaces list --plane admin --output json |
  jq '.[] | {name, slug, scopeName}'
```

### Relationship change would remove existing bindings

Stop and re-run with `--preserve-existing` when the intent is to add or disable only the named
workspace. Omit it only when replacing the complete relationship set is intentional.
