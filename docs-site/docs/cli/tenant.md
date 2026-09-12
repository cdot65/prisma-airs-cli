---
title: Tenant selection
sidebar_label: tenant
---

# Tenant selection

Tenants are the **only** configuration source. Create a tenant through guided prompts or
register an existing Prisma AIRS JSON file, select it, and every command reads that file.
Existing files can be read-only: registering, switching, reading, and deleting registrations
never modify the source configuration or copy its credentials.

## Set up without a JSON file

```bash
airs tenant create development
# Prompts, one at a time: TSG ID, OAuth client ID, and a hidden OAuth client secret.
airs tenant switch development
airs tenant set development defaultOutput yaml
airs tenant set development scanConcurrency 3
airs tenant set development airsApiKey
# Hidden prompt for the runtime scanning key.
airs tenant read
```

`create` waits for all three required values before saving anything. Ctrl+C cancels
setup without registering a partial tenant (exit 130). This configures access to an
existing cloud tenant; it does not provision a tenant or service account.

`set <name> <key> [value]` changes just one setting in that tenant's file, without
changing the active selection. Omit the value to be prompted. Credentials use hidden
prompts and cannot be passed as command-line arguments. Values are schema-validated;
unrelated fields are preserved. Credential fields cannot be cleared. The registered
`mgmtTsgId` is pinned: create another tenant to use a different TSG.

`unset <name> <key>` removes one setting so the default applies again; `get <name> <key>`
prints one value with credentials redacted; `path [name]` prints the file path.

For automation, pipe a secret from your secret manager or private file:

```bash
airs tenant create development --tsg-id 100 --client-id client-100 \
  --client-secret-stdin < /secure/oauth-secret.txt
airs tenant set development mgmtClientSecret --stdin < /secure/rotated-secret.txt
```

Stdin accepts one nonempty value, up to 64 KiB, with an optional final newline.
Without a terminal, supply the creation IDs and `--client-secret-stdin`, or use
`--config`. Do not combine `--config` with new-config options. Neither creation nor
editing tests OAuth access; a successful save confirms local configuration only. Run
`airs doctor` for that.

New configs are stored under `configs/` alongside the tenant registry, with a unique
filename, directory mode `0700`, and file mode `0600` on POSIX. Secrets are stored in
that private JSON file, **not encrypted**, and never in the registry. Back up and
protect these files. `tenant set` and `tenant unset` require a writable regular file and
parent directory; they do not bypass read-only permissions. Updates use a per-config lock
and atomic replacement with mode `0600`. Deleting a registration retains even CLI-created
configs.

## Use an existing JSON file

```bash
airs tenant create development --config /secure/development.json
airs tenant create production --config /secure/production.json
airs tenant list --output json
airs tenant switch development
airs tenant read
airs tenant read production --output yaml
airs tenant delete production --force
```

`create --config` registers an **existing file**, not a new cloud tenant or service account. Each
file must contain `mgmtClientId`, `mgmtClientSecret`, and `mgmtTsgId`. Add other keys to that
same file as needed (see [configuration options](../reference/configuration.md)). A
registration does not become active until you run `switch`. Names are 1–64 letters, digits,
hyphens, or underscores, beginning with a letter or digit.

| Command | Behavior |
| --- | --- |
| `create <name>` | Prompt for TSG ID, client ID and hidden secret; create a private config |
| `create <name> --config <path>` | Validate and register an existing config; resolve its real absolute path |
| `switch <name>` | Validate its pinned TSG identity and persist the selection for subsequent CLI processes |
| `set <name> <key> [value] [--stdin]` | Update one setting; prompt if omitted, hide credentials, preserve selection |
| `unset <name> <key>` | Remove one non-credential setting so the default applies |
| `get <name> <key> --output <format>` | Print one setting, credentials redacted |
| `list --output <format>` | Show names, selected status, TSG IDs, and file paths; no credential reads |
| `read [name] --output <format>` | Show file settings with all credential values redacted; defaults to the selected tenant |
| `path [name]` | Print a tenant's config file path; defaults to the selected tenant |
| `delete <name> [--force]` | Unregister a tenant; keep its config file; clears the selection if it was selected |

`list`, `read`, and `get` support `pretty`, `table`, `markdown`, `csv`, `json`, and `yaml`.
There is intentionally no `--reveal`. A missing or invalid selected config fails closed;
`tenant list`, `tenant switch`, and `tenant delete` remain available for recovery. With no
tenant selected, every API command stops with `No tenant selected` and names the registered
tenants.

## Registry and precedence

The small, versioned registry contains paths and TSG IDs, **not credentials**. It lives at
`$XDG_STATE_HOME/prisma-airs/tenants.json`, or `~/.local/state/prisma-airs/tenants.json`.
Use `PRISMA_AIRS_TENANTS_PATH` to isolate a registry for a container, automation job,
or shell session. Updates use a private lock and atomic replacement; new directories
use mode `0700` and the registry uses `0600` on POSIX systems. Back up the registry
and its referenced config files separately. If a process leaves a lock behind, confirm
that no writer is running before removing that specific `.lock` file.

Config resolution is: CLI flags, then the selected tenant's file, then defaults. No
environment variable supplies a configuration value, and a `.env` file is not loaded.
`airs doctor` names the selected tenant and TSG, validates the file against its pinned
identity, lists any `PANW_*` or `PRISMA_AIRS_CONFIG_PATH` variables still set in the shell
(they are ignored), and phrases every remedy as `airs tenant set <name> <key>`.

Selection affects new commands, not already running processes. For parallel jobs targeting
different tenants, give each job its own registry through `PRISMA_AIRS_TENANTS_PATH`.

For disposable Docker containers, persist the registry separately from the read-only
config mount (for the published root-based image, mount a volume at
`/root/.local/state/prisma-airs`). Otherwise a selection disappears when the container
is removed. Backups likewise need an explicit path on a writable bind mount to survive
container removal. Config paths are interpreted inside the container.

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

## Guided setup acceptance output

Captured on 2026-09-08 with synthetic credentials and an isolated local registry;
this is a terminal interaction test, not a cloud authentication claim. Terminal
control sequences have been removed; no credential value is included below.

```text
$ airs tenant create guided-demo
✔ Tenant service group ID (mgmtTsgId): 100
✔ OAuth client ID (mgmtClientId): client-100
✔ OAuth client secret (mgmtClientSecret):
  ✓ Registered guided-demo (TSG 100); private config created. Use airs tenant switch guided-demo.

$ airs tenant set guided-demo defaultOutput
✔ defaultOutput: yaml
  ✓ Updated defaultOutput for tenant guided-demo; selection unchanged.

$ airs tenant create cancelled-demo --tsg-id 100 --client-id client-100
? OAuth client secret (mgmtClientSecret): [input is masked]
# Ctrl+C
  Cancelled; no configuration saved.
```

Cancellation exited 130 and left only the completed `guided-demo` registration and
its config; selection remained unchanged. `tenant read guided-demo --output json`
returned `defaultOutput: yaml` and `[REDACTED]` for `mgmtClientSecret`. Separate
built-CLI integration tests verified stdin setup, secret rotation, individual endpoint
updates, and OAuth/profile retrieval against a local HTTP test API. The existing
two-tenant profile backup/restore integration workflows also passed.
