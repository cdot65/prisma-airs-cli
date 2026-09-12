---
title: Configuration
---

# Configuration

Prisma AIRS CLI is configured entirely through **tenants**. A tenant is a private JSON file
holding one set of SCM OAuth credentials (client ID, secret, TSG ID) plus any optional
settings, registered under a name. Exactly one tenant is selected at a time, and every command
reads only that file. Environment variables are never consulted.

## Config Cascade

Settings are resolved in priority order (highest wins):

```
CLI flags  >  Selected tenant's config file  >  Zod defaults
```

## First-time setup

```bash
airs tenant create development     # prompts for TSG ID, client ID, and a hidden client secret
airs tenant switch development     # every command now uses this tenant
airs doctor                        # verify the file, credentials, and connectivity
```

For scanning, add the runtime key with a hidden prompt:

```bash
airs tenant set development airsApiKey
```

Already have a JSON file? Register it without copying or editing it:

```bash
airs tenant create production --config /secure/production.json
```

## Managing settings

`airs tenant` covers every read and write:

```bash
airs tenant read                          # all settings of the selected tenant, secrets redacted
airs tenant get development scanConcurrency
airs tenant set development scanConcurrency 3
airs tenant set development defaultOutput json
airs tenant set development mgmtClientSecret --stdin < /secure/rotated-secret.txt
airs tenant unset development defaultOutput   # defaults take over
airs tenant path                          # print the selected tenant's file path
```

- **`set`** validates the value through the schema before writing; invalid values are rejected
  and nothing is written. Credentials never go on the command line: omit the value for a hidden
  prompt or pipe it with `--stdin`. The registered `mgmtTsgId` is pinned.
- **`unset`** removes a key so the schema default applies; credential keys cannot be cleared.
- **`get`** and **`read`** redact every key whose name ends in `key`, `secret`, `token`, or
  `password`. There is intentionally no `--reveal`.
- **`path`** prints only the file path, which is pipe-friendly.

Keys and defaults are listed in [configuration options](../reference/configuration.md).
Multi-tenant workflows, automation, and the registry layout are in
[tenant selection](../cli/tenant.md).

## Tuning Parameters

| Config Key | Default | What it does |
|-----------|---------|-------------|
| `scanConcurrency` | `5` | Parallel scan requests per batch (1--20) |
| `dataDir` | `~/.prisma-airs/runs` | Bulk-scan state directory |
| `defaultOutput` | `pretty` | Default read format: `pretty`, `table`, `markdown`, `csv`, `json`, or `yaml` |

:::tip[Concurrency vs. rate limits]
Keep `scanConcurrency` at 5 or lower to avoid AIRS rate limiting. Increase only if your tenant has elevated quotas.
:::

## Data Locations

| Path | Purpose |
|------|---------|
| `~/.local/state/prisma-airs/tenants.json` | Tenant registry (names, file paths, TSG IDs; never credentials) |
| `~/.local/state/prisma-airs/configs/` | Config files created by `airs tenant create` (mode `0600`) |
| `~/.prisma-airs/runs/` | Bulk-scan state (`dataDir`) |
