# Configuration Options

Every setting in Prisma AIRS CLI — with its CLI flag, env var, and default value.

---

## Config Cascade

Settings resolve through a four-level cascade (highest priority wins):

1. **CLI flags** (`--provider`, `--model`, etc.)
2. **Environment variables** (`LLM_PROVIDER`, `LLM_MODEL`, etc.)
3. **Config file** (`~/.prisma-airs/config.json`)
4. **Zod schema defaults**

!!! info
    The `~` prefix in any path value is expanded to `$HOME` at load time.

---

## Config File

Optional JSON file at `~/.prisma-airs/config.json`. Keys use camelCase matching the Zod schema.

```json
{
  "llmProvider": "claude-api",
  "llmModel": "claude-opus-4-6",
  "scanConcurrency": 3,
  "dataDir": "~/.prisma-airs/runs"
}
```

---

## All Settings

| Setting | CLI Flag | Env Var | Default | What it does |
|---------|----------|---------|---------|-------------|
| `llmProvider` | `--provider` | `LLM_PROVIDER` | `claude-api` | LLM provider selection (used by audit) |
| `llmModel` | `--model` | `LLM_MODEL` | per-provider | Model override |
| `scanConcurrency` | -- | `SCAN_CONCURRENCY` | `5` | Parallel scan requests (1--20) |
| `dataDir` | -- | `DATA_DIR` | `~/.prisma-airs/runs` | Data directory |

### Provider Default Models

| Provider | Default Model |
|----------|--------------|
| `claude-api` | `claude-opus-4-6` |
| `claude-vertex` | `claude-opus-4-6` |
| `claude-bedrock` | `anthropic.claude-opus-4-6-v1` |
| `gemini-api` | `gemini-2.5-pro` |
| `gemini-vertex` | `gemini-2.5-pro` |
| `gemini-bedrock` | `gemini-2.5-pro` |

!!! warning "Concurrency tuning"
    `scanConcurrency` above 5 risks AIRS rate limiting. Increase cautiously.

