# Migration: daystrom → prisma-airs-cli

This project was renamed from `daystrom` to `prisma-airs-cli` in March 2026.

## What Changed

| Aspect | Before | After |
|--------|--------|-------|
| Package name | `@cdot65/daystrom` | `@cdot65/prisma-airs-cli` |
| CLI binary | `daystrom` | `airs` |
| Data directory | `~/.daystrom/` | `~/.prisma-airs/` |
| Config file | `~/.daystrom/config.json` | `~/.prisma-airs/config.json` |
| Memory directory | `~/.daystrom/memory/` | `~/.prisma-airs/memory/` |
| Run state | `~/.daystrom/runs/` | `~/.prisma-airs/runs/` |
| Docker image | `cdot65/daystrom` | `cdot65/prisma-airs-cli` |
| GitHub repo | `cdot65/daystrom` | `cdot65/prisma-airs-cli` |
| Docs site | `cdot65.github.io/daystrom` | `cdot65.github.io/prisma-airs-cli` |

## Steps for Existing Users

### 1. Install the new package

```bash
# Global install
npm uninstall -g @cdot65/daystrom
npm install -g @cdot65/prisma-airs-cli

# Or with pnpm
pnpm remove -g @cdot65/daystrom
pnpm add -g @cdot65/prisma-airs-cli
```

### 2. Migrate local data

Copy your existing runs, memory, and config to the new data directory:

```bash
cp -r ~/.daystrom/ ~/.prisma-airs/
```

### 3. Update shell aliases and scripts

Replace all `daystrom` CLI invocations with `airs`:

```bash
# Before
daystrom generate --provider claude-api --profile my-profile
daystrom runtime scan --profile my-profile "test prompt"

# After
airs generate --provider claude-api --profile my-profile
airs runtime scan --profile my-profile "test prompt"
```

### 4. Update environment variables

No environment variable names changed. Only default paths in config changed:

- `DATA_DIR` default: `~/.daystrom/runs` → `~/.prisma-airs/runs`
- `MEMORY_DIR` default: `~/.daystrom/memory` → `~/.prisma-airs/memory`

If you had explicit `DATA_DIR` or `MEMORY_DIR` overrides, they continue to work as-is.

### 5. Update Docker usage

```bash
# Before
docker run --rm -v ~/.daystrom:/root/.daystrom cdot65/daystrom

# After
docker run --rm -v ~/.prisma-airs:/root/.prisma-airs cdot65/prisma-airs-cli
```

## CLI Command Changes (v1.14 → v1.0.0 fresh)

In addition to the rename, guardrail and audit commands were nested under `runtime`:

| Before | After |
|--------|-------|
| `daystrom generate` | `airs runtime topics generate` (alias: `airs generate`) |
| `daystrom resume` | `airs runtime topics resume` (alias: `airs resume`) |
| `daystrom report` | `airs runtime topics report` (alias: `airs report`) |
| `daystrom list` | `airs runtime topics runs` (alias: `airs list`) |
| `daystrom audit` | `airs runtime profiles audit` (alias: `airs audit`) |

Top-level aliases (`airs generate`, `airs resume`, etc.) are deprecated and will show a warning.

## Archived Repository

The original `cdot65/daystrom` repository is archived and read-only. All future development continues at `cdot65/prisma-airs-cli`.
