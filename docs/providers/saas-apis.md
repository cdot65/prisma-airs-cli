# SaaS APIs

The simplest way to get started. The `claude-api` and `gemini-api` providers connect directly to Anthropic and Google AI Studio — all you need is an API key.

---

## claude-api

Direct connection to the Anthropic API via `@langchain/anthropic`.

### Environment Variables

| Env Var | Required | Description |
|---------|----------|-------------|
| `ANTHROPIC_API_KEY` | Yes | Anthropic API key (`sk-ant-...`) |

### Setup

=== "Environment File"

    ```bash title=".env"
    LLM_PROVIDER=claude-api
    ANTHROPIC_API_KEY=sk-ant-api03-xxxxxxxxxxxx
    ```

=== "Export"

    ```bash
    export LLM_PROVIDER=claude-api
    export ANTHROPIC_API_KEY=sk-ant-api03-xxxxxxxxxxxx
    ```

### Usage

```bash
# Uses defaults from .env / config
pnpm run generate

# Explicit provider flag
pnpm run generate -- --provider claude-api

# Override model
pnpm run generate -- --provider claude-api --model claude-sonnet-4-20250514
```

!!! tip "Getting an API key"

    Create an API key at [console.anthropic.com](https://console.anthropic.com/). Keys start with `sk-ant-`.

---

## gemini-api

Direct connection to the Google AI Studio API via `@langchain/google-genai`.

### Environment Variables

| Env Var | Required | Description |
|---------|----------|-------------|
| `GOOGLE_API_KEY` | Yes | Google AI API key |

### Setup

=== "Environment File"

    ```bash title=".env"
    LLM_PROVIDER=gemini-api
    GOOGLE_API_KEY=AIzaSyXXXXXXXXXXXXXXXXXXXXXXXXXXXXX
    ```

=== "Export"

    ```bash
    export LLM_PROVIDER=gemini-api
    export GOOGLE_API_KEY=AIzaSyXXXXXXXXXXXXXXXXXXXXXXXXXXXXX
    ```

### Usage

```bash
# Uses defaults from .env / config
pnpm run generate

# Explicit provider flag
pnpm run generate -- --provider gemini-api

# Override model
pnpm run generate -- --provider gemini-api --model gemini-2.5-pro
```

!!! tip "Getting an API key"

    Create an API key at [aistudio.google.com](https://aistudio.google.com/). Keys start with `AIza`.

---

## Common Notes

!!! info "Rate Limits"

    Both SaaS APIs have per-minute and per-day rate limits. If you hit `429` errors during scan batches, reduce `SCAN_CONCURRENCY` (default: 5) or add delay between runs.

!!! warning "API Key Security"

    Never commit API keys to version control. Use `.env` files (already in `.gitignore`) or a secrets manager.
