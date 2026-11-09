---
title: Upgrading from the TypeScript CLI (7.x)
sidebar_label: Upgrading from 7.x
sidebar_position: 2
---

# Upgrading from the TypeScript CLI (7.x)

From 8.0.0, `@cdot65/prisma-airs-cli` and this repository ship the Rust CLI. It replaces
the TypeScript CLI (7.x, end of life) outright: there is no TypeScript code in 8.0.0 and no
dispatcher between the two. The command is now `airs`; `airs-cli` still works and runs the
same program. Behaviour that differs from the TypeScript CLI is listed, row by row, in
`docs/DESIGN.md` §12 in the repository, each row pinned by a test.

## Steps

1. **Install 8.0.0** through one channel ([Install](./install.md)). An npm install
   upgrades in place:

   ```sh
   npm install -g @cdot65/prisma-airs-cli@latest
   ```

   With Homebrew, cargo or a tarball, uninstall the TypeScript package from every Node
   version first (`npm uninstall -g @cdot65/prisma-airs-cli`, and `pnpm remove -g` where
   pnpm installed it), or its `airs-cli` keeps answering.

2. **Register your tenants.** The Rust CLI keeps its own tenant store and never reads the
   TypeScript registry:

   ```sh
   airs tenant create <name>   # prompts for the TSG ID, client ID and secret
   airs tenant switch <name>
   airs doctor
   ```

   `airs tenant import`, which copied a TypeScript registry, was removed after 8.2. With
   many tenants, install 8.2 once to run its `tenant import`, then upgrade.

3. **Finish or abandon TypeScript bulk-scan jobs.** Their checkpoints are not imported.

4. **Take new backups.** Backups made by the TypeScript CLI (runtime profiles, DLP, red
   team targets) are not read by the Rust CLI; back up again with `airs`.

5. **Update scripts** for the changes below.

## Tenant keys

A tenant file must use the new names:

| TypeScript | Rust |
|---|---|
| `aiGw*` (for example `aiGwDataEndpoint`) | `gateway*` (`gatewayDataEndpoint`) |
| `agentGuard*` | `supplychain*` |
| `dataDir`, the `typesafe*` keys, the retired per-product `*TokenEndpoint` keys (`mgmtTokenEndpoint` stays) | dropped |
| `defaultOutput` of `markdown`, `csv` or `yaml` | invalid in a tenant file |
| empty values, unknown keys | an unknown key makes a tenant file invalid |
| numbers as strings or JSON numbers | plain decimal integers |

## Renamed command groups

| TypeScript | Rust |
|---|---|
| `airs-cli aigateway …` | `airs gateway …` |
| `airs-cli agentguard …` | `airs supplychain …` |

There are no aliases for the old group names. The doctor's `AI Gateway API` check is now
`Gateway API`.

## Dropped features

| Dropped | Instead |
|---|---|
| `completion` | none |
| `redteam judge` and the TypeSafe credentials and checks | none (it was a proof of concept) |
| `runtime dlp generate` | none |
| `--output markdown`, `csv` and `yaml` | `pretty`, `table` or `json`; anything else is a usage error (exit 2) |
| the doctor's Node.js and TypeSafe checks | the `Executable` check reports which `airs` runs |
| `tenant create --config <file>` | `airs tenant create <name>` |
| credentials and endpoints from environment variables (`PANW_*`, `PRISMA_AIRS_CONFIG_PATH` and the other retired names) | tenant settings; `airs doctor` lists the variables that are still set |
| reading TypeScript backup files and bulk-scan checkpoints | new backups and jobs made with `airs` |

## What changes for scripts

| Change | Detail |
|---|---|
| Command name | `airs`. `airs-cli` and `airs-cli-rs` run the same program; help texts, hints and examples name `airs`, so scripts that match `airs-cli` in stderr text change accordingly. |
| Exit codes | Argument-parse errors exit 2, not 1, and usage is checked before the tenant is read. SIGINT and SIGTERM give 130 and 143. |
| Scans fail closed | A failed scan prints `FAILED` and exits 1; TypeScript printed `ALLOW` and exited 0. Response detections count. A scan is never re-sent. |
| Writes | Every write is sent once. A write whose outcome is unknown says so instead of being retried. |
| Environment | No `PANW_*` credential fallback; timeout and debug variables are ignored. `HTTPS_PROXY` and macOS system proxies are honoured. Plain-http endpoints other than loopback are refused. |
| Doctor | Its own checks; server errors fail and rate limits warn. Each probe makes one attempt. |
| Error text | Built from SDK metadata. Secrets and tokens are withheld. |
| Pagination | A walk that cannot finish fails instead of printing a truncated list. |
| State locations | Tenants in `~/.config/airs-cli/`; bulk-scan state and debug logs in `~/.local/state/airs-cli/`. `PRISMA_AIRS_TENANTS_PATH` no longer selects tenants. |
| Tenant commands | `tenant delete` also deletes the tenant's file. `tenant set <name> mgmtTsgId <tsg>` changes the TSG and its pin. Success lines go to stderr and `--quiet` hides them. Input errors exit 2. Names that differ only in case are refused. |
| Concurrent changes | Tenant writers wait up to 5 s for each other and name the holder; a crashed writer never leaves a lock behind. |
| `--debug` | The log goes to `~/.local/state/airs-cli/debug/`, not the current directory; only the newest 20 are kept. It records every request, its status and allow-listed headers only: no bodies and no URL queries. |

See [Output, exit codes and the debug log](./output-and-exit-codes.md) for the details.
