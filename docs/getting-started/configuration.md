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

This means a `--provider` flag always beats an env var, which always beats the config file.

## Config File

For settings you use across every run, create `~/.prisma-airs/config.json`:

```json title="~/.prisma-airs/config.json"
{
  "llmProvider": "claude-api",
  "scanConcurrency": 5,
  "maxMemoryChars": 3000,
  "memoryEnabled": true
}
```

## LLM Providers

| Provider | Config Value | Default Model | Auth |
|----------|-------------|---------------|------|
| Claude API | `claude-api` | `claude-opus-4-6` | `ANTHROPIC_API_KEY` |
| Claude Vertex | `claude-vertex` | `claude-opus-4-6` | GCP ADC |
| Claude Bedrock | `claude-bedrock` | `anthropic.claude-opus-4-6-v1` | AWS creds |
| Gemini API | `gemini-api` | `gemini-2.5-pro` | `GOOGLE_API_KEY` |
| Gemini Vertex | `gemini-vertex` | `gemini-2.5-pro` | GCP ADC |
| Gemini Bedrock | `gemini-bedrock` | `gemini-2.5-pro` | AWS creds |

!!! note "Claude Vertex region"
    The `claude-vertex` provider defaults to the `global` region, not `us-central1`. Override with `GOOGLE_CLOUD_LOCATION` if needed.

For detailed provider setup, see [LLM Providers](../providers/overview.md).

## Tuning Parameters

These settings control how Prisma AIRS CLI interacts with AIRS and the memory system.

| Env Var | Config Key | Default | What it does |
|---------|-----------|---------|-------------|
| `SCAN_CONCURRENCY` | `scanConcurrency` | `5` | Parallel scan requests per batch (1--20) |
| `MAX_MEMORY_CHARS` | `maxMemoryChars` | `3000` | Character budget for memory injection (500--10000) |
| `MEMORY_ENABLED` | `memoryEnabled` | `true` | Toggle cross-run learning on/off |
| `ACCUMULATE_TESTS` | `accumulateTests` | `false` | Carry forward tests across iterations |
| `MAX_ACCUMULATED_TESTS` | `maxAccumulatedTests` | unlimited | Cap on accumulated test count |
| `DATA_DIR` | `dataDir` | `~/.prisma-airs/runs` | Where run states are saved |
| `MEMORY_DIR` | `memoryDir` | `~/.prisma-airs/memory` | Where learnings are stored |

!!! tip "Concurrency vs. rate limits"
    Keep `scanConcurrency` at 5 or lower to avoid AIRS rate limiting. Increase only if your tenant has elevated quotas.

## Data Locations

| Path | Purpose |
|------|---------|
| `~/.prisma-airs/config.json` | Persistent configuration |
| `~/.prisma-airs/runs/` | Saved run states (JSON per run) |
| `~/.prisma-airs/memory/` | Cross-run learnings (JSON per category) |
