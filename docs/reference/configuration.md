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
  "maxMemoryChars": 5000,
  "memoryEnabled": true,
  "accumulateTests": false,
  "dataDir": "~/.prisma-airs/runs",
  "memoryDir": "~/.prisma-airs/memory"
}
```

---

## All Settings

| Setting | CLI Flag | Env Var | Default | What it does |
|---------|----------|---------|---------|-------------|
| `llmProvider` | `--provider` | `LLM_PROVIDER` | `claude-api` | LLM provider selection |
| `llmModel` | `--model` | `LLM_MODEL` | per-provider | Model override |
| `scanConcurrency` | -- | `SCAN_CONCURRENCY` | `5` | Parallel scan requests (1--20) |
| `maxMemoryChars` | -- | `MAX_MEMORY_CHARS` | `3000` | Memory injection budget (500--10000) |
| `memoryEnabled` | `--no-memory` | `MEMORY_ENABLED` | `true` | Toggle cross-run learning |
| `accumulateTests` | `--accumulate-tests` | `ACCUMULATE_TESTS` | `false` | Carry forward tests across iterations |
| `maxAccumulatedTests` | `--max-accumulated-tests` | `MAX_ACCUMULATED_TESTS` | unlimited | Cap on accumulated test count |
| `dataDir` | -- | `DATA_DIR` | `~/.prisma-airs/runs` | Run state directory |
| `memoryDir` | -- | `MEMORY_DIR` | `~/.prisma-airs/memory` | Learning store directory |

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

