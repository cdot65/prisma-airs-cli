---
title: AI Gateway CLI command plan
sidebar_position: 8
---

# AI Gateway CLI command plan

This document maps `@cdot65/prisma-airs-sdk` 0.19.0 onto a stable `airs aigateway` command tree.
It is an implementation plan: commands marked **current** exist today; commands marked **planned**
must be delivered test-first and live-validated before they are documented as available.

## Design rules

- Keep every command group and help listing alphabetically sorted.
- Use resource nouns that match the SDK: plural for collections, except the existing
  backward-compatible `workspace` group.
- All reads support `--output pretty|table|markdown|csv|json|yaml` where the shape permits it.
- Structured stdout contains data only. Status, warnings, and confirmation prompts use stderr.
- Create/update payloads use explicit flags for common fields and `--file <json|yaml>` for nested
  service-specific bodies. Command flags override file fields.
- `delete`, archive, revoke, rotation, and binding replacement require confirmation; `--force`
  permits non-interactive execution.
- Provider details and one-time API-key/deployment credentials are redacted by default. Revealing
  or writing them requires an explicit secret-safe option and owner-only file permissions.
- Preserve Prisma's split service/user API-key collections. Do not add a generic API-key route.
- A private deployment can have a healthy outbound heartbeat while `deployments ping` fails because
  the optional diagnostic requires control-plane-initiated ingress.

## Proposed hierarchy

```text
airs aigateway
├── api-keys
│   ├── service  list · get · create · update · delete · rotate
│   └── user     list · get · create · update · delete · rotate
├── audit-logs   list
├── configs      list · get · versions · create · update · delete
├── deployments  list · get · create · update · archive · ping
├── guardrails   list · get · create · update · delete
├── integrations list · get · create · update · delete
│                models get · set
│                workspaces get · set
├── mcp-integrations list · get · create · update · delete
│                    capabilities get · set
│                    metadata get
│                    workspaces get · set
├── organisations self get · update
│                 auth-settings get · update
├── plugins       list · create
├── providers     list · get · create · update · delete
├── telemetry     cost · requests · latency · tokens · errors · users
│                 cache-summary · cache-trend · user-trends · error-trends
│                 rescued-retries · feedback-* · group-by · status-codes · logs
└── workspace     list · get · create · update · delete
```

Top-level groups should render alphabetically: `api-keys`, `audit-logs`, `configs`, `deployments`,
`guardrails`, `integrations`, `mcp-integrations`, `organisations`, `plugins`, `providers`,
`telemetry`, `workspace`.

## Coverage matrix

| CLI group | SDK mapping | State | Notes |
| --- | --- | --- | --- |
| `workspace` | `gw.workspaces.*` | **Current** | Full lifecycle; delete archives. |
| `telemetry cost` | `gw.telemetry.cost()` | **Current** | Wire values are cents; pretty output converts to dollars. |
| Remaining telemetry | `gw.telemetry.*` | Planned read | Share window flags and renderers. |
| `configs` | `gw.configs.*` | Planned CRUD | `versions` maps to `listVersions()`. Hard delete. |
| `guardrails` | `gw.guardrails.*` | Planned CRUD | Hard delete. Nested checks/actions favor file input. |
| `providers` | `gw.providers.*` | Planned CRUD | Detail may contain credentials; redact by default. Hard delete. |
| `api-keys service` | `gw.apiKeys.*Service()` | Planned CRUD | Create/rotate return one-time secrets. |
| `api-keys user` | `gw.apiKeys.*User()` | Planned CRUD | Requires user-specific fields. |
| `integrations` | `gw.integrations.*` | Planned CRUD | Include model and workspace binding subcommands. |
| `mcp-integrations` | `gw.mcpIntegrations.*` | Planned CRUD | Include metadata, capabilities, and workspace bindings. |
| `deployments` | `gw.deployments.*` | Planned CRUD | Delete is named `archive`; create returns one-time credentials. |
| `plugins` | `gw.plugins.list/create` | Planned partial | SDK has no verified get/update/delete yet. |
| `organisations` | `gw.organisations.*` | Planned read/update | No destructive organisation lifecycle. |
| `audit-logs` | `gw.auditLogs.list()` | Planned read | Never print unredacted request bodies by default. |

## Command contracts

### Read conventions

```bash
airs aigateway configs list --workspace <uuid> --output json
airs aigateway configs get <config-id> --output yaml
airs aigateway configs versions <config-id> --output table
airs aigateway mcp-integrations capabilities get <integration-id> --output json
airs aigateway deployments ping <deployment-id> --output json
```

Workspace-scoped configuration lists take the workspace UUID. Telemetry takes the workspace slug.
The CLI may resolve a unique workspace name, but output and errors must state which identifier was
used.

### Mutation conventions

```bash
airs aigateway configs create --file config.json --output json
airs aigateway providers update <provider-id> --file provider.json
airs aigateway mcp-integrations workspaces set <integration-id> \
  --workspace ws-development=disabled --preserve-existing --force
airs aigateway api-keys service rotate <key-id> \
  --transition-ms 1800000 --output-file ./rotated-key.json --force
airs aigateway deployments archive <deployment-id> --organisation-id <tsg> --force
```

`--preserve-existing` maps to `override_existing_workspace_access: false`; replacement must be an
explicitly named option. API-key rotation and deployment registration output files are created with
mode `0600` and must never be included in debug logs.

## Delivery phases

1. **Read inventory:** configs, guardrails, providers, API keys, integrations, MCP integrations,
   deployments, plugins, organisations, audit logs, and remaining telemetry reads.
2. **Low-risk writes:** config/guardrail/provider create and update; integration and MCP update;
   capability and binding writes with read-back verification.
3. **Secret-bearing writes:** API-key create/rotate and deployment registration with protected-file
   output and log-redaction tests.
4. **Destructive lifecycle:** hard deletes, revocation, and deployment archive with confirmations,
   exact-target read-back, aliases only where semantics match (`rm` must not disguise archive).
5. **Documentation and E2E:** command reference, examples reflecting default output behavior,
   completion scripts, help-order assertions, live-safe read suite, and disposable mutation cycles.

## Definition of done

- Every command begins with a failing command-tree or handler test.
- Help text and subcommands are alphabetically sorted and covered by tests.
- SDK request mapping, validation, stdout/stderr separation, formats, and exit codes are tested.
- Secret-bearing responses are redacted by default and never enter debug logs.
- Destructive commands require confirmation and identify hard delete versus archive/revoke.
- Docusaurus, README, `AGENTS.md`, completions, and actual `--help` output agree.
- Full CLI validation passes against the installed SDK version before release.
