---
title: AI Gateway CLI command plan
sidebar_position: 8
---

# AI Gateway CLI command plan

This document maps `@cdot65/prisma-airs-sdk` 0.20.0 onto a stable `airs aigateway` command tree.
The command surface described here is implemented. It remains the design contract for future AI
Gateway additions and records the test-first delivery phases used for the SDK 0.20.0 expansion.

## Design rules

- Keep every command group and help listing alphabetically sorted.
- Use one predictable grammar: `airs aigateway <resource> <verb> [ref]`. Relationship resources
  add one level: `<resource> <relationship> <verb>`.
- Use plural collection nouns. Make `workspaces` canonical and retain the existing `workspace`
  spelling as a hidden compatibility alias.
- Reserve `mcp` as a namespace. It contains `integrations` now and can add the SDK's planned
  `servers` resource later without inventing another top-level spelling.
- All reads support `--output pretty|table|markdown|csv|json|yaml` where the shape permits it.
- Structured stdout contains data only. Status, warnings, and confirmation prompts use stderr.
- Create/update payloads use explicit flags for stable fields and repeatable `--set` /
  `--set-string` dotted values for nested service-specific bodies. Optional `--file <json|yaml>` is
  an advanced base; named and dotted flags override file fields.
- `delete`, archive, revoke, rotation, and binding replacement require confirmation; `--force`
  permits non-interactive execution.
- Provider details and one-time API-key/deployment credentials are redacted by default. Revealing
  or writing them requires an explicit secret-safe option and owner-only file permissions.
- Preserve Prisma's split service/user API-key collections. Do not add a generic API-key route.
- A private deployment can have a healthy outbound heartbeat while `deployments ping` fails because
  the optional diagnostic requires control-plane-initiated ingress.

## Operator grammar

The command immediately after the resource is always an action:

| Intent | Verb | Rule |
| --- | --- | --- |
| Browse a collection | `list` (`ls`) | Never requires a positional id. |
| Inspect one item | `get` | Takes one UUID or documented slug/ref. |
| Add an item | `create` | Stable fields as flags; nested bodies through repeatable `--set`. |
| Change an item | `update` | Partial update unless help explicitly says replacement. |
| Replace a relationship | `set` | Help states whether existing bindings are preserved. |
| Permanently remove | `delete` (`rm`) | Hard delete only; always confirmed. |
| Soft-remove | `archive` | Never given the misleading `rm` alias. |
| Replace a credential | `rotate` | Explicit secret destination and confirmation required. |
| Check connectivity | `ping` | Reports heartbeat and ping as separate signals. |

`delete` must never secretly mean archive. The current `workspace delete` spelling remains as a
deprecated compatibility path, but help presents `workspaces archive` as canonical.

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
│                models list · set
│                workspaces list · set
├── mcp
│   └── integrations list · get · create · update · delete
│                    capabilities list · set
│                    metadata get
│                    workspaces list · set
├── organisations self get · update
│                 auth-settings get · update
├── plugins       list · create
├── providers     list · get · create · update · delete
├── telemetry
│   ├── cache       summary · trend
│   ├── feedback    distribution · models · trend · weighted
│   ├── cost · errors · latency · requests · tokens · users
│   ├── error-trends · rescued-retries · user-trends
│   ├── group-by <dimension>
│   └── logs list
└── workspaces     list · get · create · update · archive
```

Top-level groups should render alphabetically: `api-keys`, `audit-logs`, `configs`, `deployments`,
`guardrails`, `integrations`, `mcp`, `organisations`, `plugins`, `providers`, `telemetry`,
`workspaces`.

### Compatibility aliases

Keep aliases intentionally small so help and completion remain teachable:

- `workspace` → `workspaces` (hidden compatibility alias).
- `workspace delete` → `workspaces archive` (deprecated compatibility path with a warning).
- `list` → `ls` everywhere.
- `delete` → `rm` only for true hard deletes.

Do not add aliases such as `show`, `describe`, `edit`, or `remove`; they multiply vocabulary
without adding capability.

## Help and navigation contract

Every level must be useful without prior documentation:

```bash
airs aigateway --help
airs aigateway help mcp
airs aigateway mcp --help
airs aigateway mcp integrations --help
airs aigateway mcp integrations workspaces set --help
```

Invoking a group without a leaf action prints that group's help and exits `0`. Unknown commands
remain usage errors (`2`) and show Commander's nearest-command suggestion.

Help content follows the same order at every level:

1. One-sentence purpose, including **data plane** or **admin plane**.
2. Usage line.
3. Alphabetically sorted options and subcommands.
4. Identifier note: workspace UUID/slug/name or resource UUID, as applicable.
5. Side-effect warning for replacement, secret-bearing, destructive, or archive operations.
6. Two or three copyable examples: human output first, JSON automation second, mutation last.
7. A short “See also” line for the closest related command when useful.

Top-level help stays compact. It lists resource groups only; detail belongs one level down.
Descriptions begin with a verb and avoid repeating “AI Gateway.”

### Suggested top-level help

```text
Usage: airs aigateway [options] [command]

Manage and observe Prisma AIRS AI Gateway resources

Commands:
  api-keys       Manage service and user gateway credentials
  audit-logs     Inspect organisation audit activity
  configs        Manage routing configurations
  deployments    Manage self-hosted gateway registrations
  guardrails     Manage workspace guardrails
  integrations   Manage organisation provider integrations
  mcp            Manage MCP integrations and servers
  organisations  Manage organisation and authentication settings
  plugins        Manage gateway plugins
  providers      Manage workspace provider bindings
  telemetry      Inspect gateway usage and request telemetry
  workspaces     Manage gateway workspaces
```

## Implementation layout

Do not grow the current `src/cli/commands/aigateway.ts` into a multi-thousand-line command file.
Keep registration and shared behavior separate from resource handlers:

```text
src/cli/commands/aigateway/
├── index.ts              root registration and canonical ordering
├── shared.ts             workspace resolution, windows, files, secrets, confirmations
├── api-keys.ts
├── audit-logs.ts
├── configs.ts
├── deployments.ts
├── guardrails.ts
├── integrations.ts
├── mcp.ts
├── organisations.ts
├── plugins.ts
├── providers.ts
├── telemetry.ts
└── workspaces.ts
```

The command tree must be constructible without loading credentials or making requests. Service
construction stays inside leaf actions so `--help`, completions, and command-tree tests remain fast
and offline. Renderer modules should be resource-focused; raw SDK objects do not leak directly into
pretty/table output.

Shared helpers own these cross-cutting contracts:

- workspace name/slug/UUID resolution;
- positive integer, date window, enum, and JSON/YAML file validation;
- `--output` resolution and stdout/stderr separation;
- confirmation and `--force` behavior;
- redaction and owner-only secret file writes;
- consistent grant hints for data-plane versus admin-plane 403 responses.

## Coverage matrix

| CLI group | SDK mapping | State | Notes |
| --- | --- | --- | --- |
| `workspaces` (`workspace`) | `gw.workspaces.*` | **Current** | Full lifecycle; canonical delete spelling becomes `archive`. |
| `telemetry cost` | `gw.telemetry.cost()` | **Current** | Wire values are cents; pretty output converts to dollars. |
| Remaining telemetry | `gw.telemetry.*` | **Current** | Shares window flags and structured renderers. |
| `configs` | `gw.configs.*` | **Current** | `versions` maps to `listVersions()`. Hard delete. |
| `guardrails` | `gw.guardrails.*` | **Current** | Hard delete. Nested checks/actions use file input. |
| `providers` | `gw.providers.*` | **Current** | Detail credentials redact by default. Hard delete. |
| `api-keys service` | `gw.apiKeys.*Service()` | **Current** | Create/rotate require explicit secret output. |
| `api-keys user` | `gw.apiKeys.*User()` | **Current** | Requires user-specific fields. |
| `integrations` | `gw.integrations.*` | **Current** | Includes model and workspace binding subcommands. |
| `mcp integrations` | `gw.mcpIntegrations.*` | **Current** | Includes metadata, capabilities, and workspace bindings. |
| `deployments` | `gw.deployments.*` | **Current** | Delete is named `archive`; create returns one-time credentials. |
| `plugins` | `gw.plugins.list/create` | **Current partial** | SDK has no verified get/update/delete yet. |
| `organisations` | `gw.organisations.*` | **Current** | No destructive organisation lifecycle. |
| `audit-logs` | `gw.auditLogs.list()` | **Current** | Sensitive request fields redact by default. |

## Command contracts

### Read conventions

```bash
airs aigateway configs list --workspace <uuid> --output json
airs aigateway configs get <config-id> --output yaml
airs aigateway configs versions <config-id> --output table
airs aigateway mcp integrations capabilities list <integration-id> --output json
airs aigateway deployments ping <deployment-id> --output json
```

Workspace-scoped configuration lists take the workspace UUID. Telemetry takes the workspace slug.
The CLI may resolve a unique workspace name, but output and errors must state which identifier was
used.

### Mutation conventions

```bash
airs aigateway configs create --name primary --workspace <uuid> \
  --set config.retry.attempts=3 --output json
airs aigateway providers update <provider-id> --name vertex-primary
airs aigateway mcp integrations workspaces set <integration-id> \
  --workspace-binding ws-development=false --preserve-existing --force
airs aigateway api-keys service rotate <key-id> \
  --transition-ms 1800000 --secret-output ./rotated-key.json --force
airs aigateway deployments archive <deployment-id> --organisation-id <tsg> --force
```

`--preserve-existing` maps to `override_existing_workspace_access: false`; replacement must be an
explicitly named option. API-key rotation and deployment registration output files are created with
mode `0600` and must never be included in debug logs.

### Identifier conventions

- `--workspace <ref>` accepts a unique name, slug, or UUID through one shared resolver.
- Telemetry ultimately sends a workspace slug; config resources ultimately send a workspace UUID.
- Resource detail and mutation commands take the resource UUID as a positional argument.
- Ambiguous workspace names fail with exit code `2` and list matching slugs; the CLI never guesses.

### Secret output conventions

Secret-bearing commands require one of these explicit choices:

```bash
--secret-output <path>  # recommended; creates an owner-only 0600 file
--show-secret           # opt-in stdout for deliberate piping
```

Without either option, create/rotate refuses before making the request so a one-time credential is
not irretrievably discarded. `--debug` must redact the same fields. Provider detail and audit-log
request bodies remain redacted unless `--reveal-sensitive` is explicitly supplied.

## Delivery phases

1. **Navigation foundation:** shared group-help behavior, compatibility aliases, sorted-help tests,
   identifier resolution, and reusable read/mutation option builders.
2. **Read inventory:** configs, guardrails, providers, API keys, integrations, MCP integrations,
   deployments, plugins, organisations, audit logs, and remaining telemetry reads.
3. **Low-risk writes:** config/guardrail/provider create and update; integration and MCP update;
   capability and binding writes with read-back verification.
4. **Secret-bearing writes:** API-key create/rotate and deployment registration with protected-file
   output and log-redaction tests.
5. **Destructive lifecycle:** hard deletes, revocation, and deployment archive with confirmations,
   exact-target read-back, aliases only where semantics match (`rm` must not disguise archive).
6. **Documentation and E2E:** command reference, examples reflecting default output behavior,
   completion scripts, help-order assertions, live-safe read suite, and disposable mutation cycles.

## Definition of done

- Every command begins with a failing command-tree or handler test.
- Help text and subcommands are alphabetically sorted and covered by tests.
- SDK request mapping, validation, stdout/stderr separation, formats, and exit codes are tested.
- Secret-bearing responses are redacted by default and never enter debug logs.
- Destructive commands require confirmation and identify hard delete versus archive/revoke.
- Docusaurus, README, `AGENTS.md`, completions, and actual `--help` output agree.
- Full CLI validation passes against the installed SDK version before release.
