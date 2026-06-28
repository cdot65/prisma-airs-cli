---
title: Configuration
---

# Configuration

Prisma AIRS CLI is designed to work with minimal setup. All settings have sensible defaults — only credentials are strictly required.

## Config Cascade

Settings are resolved in priority order (highest wins):

```
CLI flags  >  Environment variables  >  ~/.prisma-airs/config.json  >  Zod defaults
```

This means a CLI flag always beats an env var, which always beats the config file.

## Config File

For settings you use across every run, create `~/.prisma-airs/config.json`:

```json title="~/.prisma-airs/config.json"
{
  "scanConcurrency": 5
}
```

## Tuning Parameters

These settings control how Prisma AIRS CLI interacts with AIRS.

| Env Var | Config Key | Default | What it does |
|---------|-----------|---------|-------------|
| `SCAN_CONCURRENCY` | `scanConcurrency` | `5` | Parallel scan requests per batch (1--20) |
| `DATA_DIR` | `dataDir` | `~/.prisma-airs/runs` | Data directory |

:::tip[Concurrency vs. rate limits]
Keep `scanConcurrency` at 5 or lower to avoid AIRS rate limiting. Increase only if your tenant has elevated quotas.
:::

## Data Locations

| Path | Purpose |
|------|---------|
| `~/.prisma-airs/config.json` | Persistent configuration |
| `~/.prisma-airs/runs/` | Data directory |
