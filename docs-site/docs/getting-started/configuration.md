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

## Managing config from the CLI

The `airs config` command group manages `~/.prisma-airs/config.json` without hand-editing:

```bash
airs config list                     # Effective config: every key, value, and source (env/file/default)
airs config get scanConcurrency      # Print a single effective value
airs config set scanConcurrency 3    # Validate via schema and write to the config file
airs config unset scanConcurrency    # Remove the key from the file (defaults take over)
airs config path                     # Print the config file path (pipe-friendly)
```

- **`list`** shows the full effective configuration with a `Source` column so you can see where each value comes from in the cascade. Supports `--output pretty|json|yaml`.
- **`set`** validates the resulting config through the schema before writing — invalid values (for example `scanConcurrency` above 20) are rejected with exit code 2 and nothing is written. Unknown keys already present in the file are preserved.
- **`unset`** removes a key from the file; if the key is not set, the command is a no-op.
- **`path`** prints only the resolved config file path, which honors the `PRISMA_AIRS_CONFIG_PATH` environment variable override.

### Secret masking and `--reveal`

Keys whose names match `key`, `secret`, `token`, or `password` (for example `airsApiKey`, `mgmtClientSecret`) are **masked** in `list` and `get` output — long values show only the last 4 characters (`***3456`), short values show `***`. Masking applies to `json` and `yaml` output too.

To print the real value, pass `--reveal`:

```bash
airs config get airsApiKey --reveal   # Prints the full value; a warning goes to stderr
airs config list --reveal             # Unmasked listing
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
