---
title: Tenant selection
sidebar_label: tenant
---

# Tenant selection

Register existing Prisma AIRS JSON configuration files and switch the CLI between them.
The files can be read-only: registering, switching, reading, and deleting registrations
never modify the source configuration or copy its credentials.

```bash
airs tenant create development --config /secure/development.json
airs tenant create production --config /secure/production.json
airs tenant list --output json
airs tenant switch development
airs tenant read
airs tenant read production --output yaml
airs tenant switch default
airs tenant delete production --force
```

`create` registers an **existing file**, not a new cloud tenant or service account. Each
file must contain `mgmtClientId`, `mgmtClientSecret`, and `mgmtTsgId`. Add product-specific
keys/endpoints to that same file as needed. A registration does not become active until
you run `switch`. Names are 1–64 letters, digits, hyphens, or underscores, beginning with
a letter or digit. `default` is reserved.

| Command | Behavior |
| --- | --- |
| `create <name> --config <path>` | Validate and register an existing config; resolve its real absolute path |
| `switch <name>` | Validate its pinned TSG identity and persist the selection for subsequent CLI processes |
| `switch default` | Return to legacy config/environment resolution |
| `list --output <format>` | Show names, selected status, TSG IDs, and file paths; no credential reads |
| `read [name] --output <format>` | Show registered-file settings, with all credential values fully redacted; defaults to selected tenant |
| `delete <name> [--force]` | Unregister an inactive tenant; keep its config file; confirmation required unless forced |

`list` and `read` support `pretty`, `table`, `markdown`, `csv`, `json`, and `yaml`.
There is intentionally no `tenant read --reveal`. Active registrations and `default`
cannot be deleted. Switch away first. A missing/invalid active config fails closed;
`tenant list` and `tenant switch default` remain available for recovery.

## Registry and precedence

The small, versioned registry contains paths and TSG IDs, **not credentials**. It lives at
`$XDG_STATE_HOME/prisma-airs/tenants.json`, or `~/.local/state/prisma-airs/tenants.json`.
Use `PRISMA_AIRS_TENANTS_PATH` to isolate a registry for a container, automation job,
or shell session. Updates use a private lock and atomic replacement; new directories
use mode `0700` and the registry uses `0600` on POSIX systems. Back up the registry
and its referenced config files separately. If a process leaves a lock behind, confirm
that no writer is running before removing that specific `.lock` file.

Config-file selection is: explicit library path, then `PRISMA_AIRS_CONFIG_PATH`, then
the selected tenant, then `~/.prisma-airs/config.json`. `switch` refuses a named selection
while `PRISMA_AIRS_CONFIG_PATH` is set, to prevent an apparently successful but ineffective
switch. If that variable is set afterward, it deliberately overrides selection and
`tenant list` warns about it.

Named tenants reject nonempty `PANW_MGMT_*`, `PANW_MODEL_SEC_*`, `PANW_RED_TEAM_*`,
`PANW_AGENT_GUARD_*`, `PANW_AI_GW_*`, `PANW_DLP_*`, and `PANW_AI_SEC_API_*` environment
overrides. Put those settings in the tenant file or unset them; this prevents mixing
one tenant's credentials with another tenant's endpoints. Output and concurrency
settings can still come from the environment. With `default`, legacy precedence remains.

Selection affects new commands, not already running processes. For parallel jobs targeting
different tenants, use separate registries or explicit `PRISMA_AIRS_CONFIG_PATH` values.
`airs config path` shows the file that API commands will use; explicit `config set/unset`
operations target that file and still require it to be writable.

## Cross-tenant profile migration

```bash
airs tenant switch development
airs runtime profiles backup --all --output-file ./profiles.json
airs tenant switch production
airs runtime profiles restore ./profiles.json --dry-run --output json
# Review the destination and dependency plan before executing:
airs runtime profiles restore ./profiles.json --expect-tsg 200 --force
```

Replace `200` with the actual destination TSG. See the
[profile migration guide](../runtime/profile-transfer.md) for DLP mappings, conflicts,
limitations, and live acceptance evidence.
