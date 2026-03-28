# Environment Variables

All environment variables Prisma AIRS CLI recognizes, grouped by category. Copy `.env.example` as a starting template.

---

## LLM Provider

| Variable | Needed for | What it does |
|----------|-----------|-------------|
| `LLM_PROVIDER` | All | Provider selection (`claude-api`, `claude-vertex`, `claude-bedrock`, `gemini-api`, `gemini-vertex`, `gemini-bedrock`) |
| `LLM_MODEL` | -- | Override the default model for any provider |
| `ANTHROPIC_API_KEY` | `claude-api` | Anthropic API key (`sk-ant-...`) |
| `GOOGLE_API_KEY` | `gemini-api` | Google AI API key |
| `GOOGLE_CLOUD_PROJECT` | `claude-vertex`, `gemini-vertex` | GCP project ID |
| `GOOGLE_CLOUD_LOCATION` | `claude-vertex`, `gemini-vertex` | GCP region (default: `us-central1`; claude-vertex uses `global`) |
| `AWS_REGION` | `claude-bedrock`, `gemini-bedrock` | AWS region (default: `us-east-1`) |
| `AWS_ACCESS_KEY_ID` | `claude-bedrock`*, `gemini-bedrock`* | IAM access key |
| `AWS_SECRET_ACCESS_KEY` | `claude-bedrock`*, `gemini-bedrock`* | IAM secret key |

!!! note
    *AWS key variables are not required if using the default credential chain (instance roles, SSO, `~/.aws/credentials`, etc.).

---

## AIRS Scan API

| Variable | Required | What it does |
|----------|:--------:|-------------|
| `PANW_AI_SEC_API_KEY` | Yes | AI Security scan API key |

---

## AIRS Management API

| Variable | Required | What it does |
|----------|:--------:|-------------|
| `PANW_MGMT_CLIENT_ID` | Yes | OAuth2 client ID |
| `PANW_MGMT_CLIENT_SECRET` | Yes | OAuth2 client secret |
| `PANW_MGMT_TSG_ID` | Yes | Tenant Service Group ID |
| `PANW_MGMT_ENDPOINT` | -- | Custom management endpoint |
| `PANW_MGMT_TOKEN_ENDPOINT` | -- | Custom token endpoint |

---

## Tuning

| Variable | Default | Range | What it controls |
|----------|---------|-------|-----------------|
| `SCAN_CONCURRENCY` | `5` | 1--20 | Parallel scan requests per batch |

!!! warning
    `SCAN_CONCURRENCY` above 5 may trigger AIRS rate limits. Increase cautiously.

---

## Paths

| Variable | Default | What it does |
|----------|---------|-------------|
| `DATA_DIR` | `~/.prisma-airs/runs` | Data directory |

!!! tip
    The `~` prefix is expanded to `$HOME` automatically.
