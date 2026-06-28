# Environment Variables

All environment variables Prisma AIRS CLI recognizes, grouped by category. Copy `.env.example` as a starting template.

---

## AIRS Scan API

| Variable | Required | What it does |
|----------|:--------:|-------------|
| `PANW_AI_SEC_API_KEY` | Yes* | AI Security scan API key |
| `PANW_AI_SEC_API_TOKEN` | Yes* | Pre-obtained bearer token (alternative to the API key) |
| `PANW_AI_SEC_API_ENDPOINT` | -- | Custom scan API endpoint (default: `service.api.aisecurity.paloaltonetworks.com`) |
| `PANW_AI_SEC_NUM_RETRIES` | -- | Max retry attempts, 0--5 (default: `5`) |

:::note
\*Provide either `PANW_AI_SEC_API_KEY` or `PANW_AI_SEC_API_TOKEN`.
:::

---

## AIRS Management API

| Variable | Required | What it does |
|----------|:--------:|-------------|
| `PANW_MGMT_CLIENT_ID` | Yes | OAuth2 client ID |
| `PANW_MGMT_CLIENT_SECRET` | Yes | OAuth2 client secret |
| `PANW_MGMT_TSG_ID` | Yes | Tenant Service Group ID |
| `PANW_MGMT_ENDPOINT` | -- | Custom management endpoint |
| `PANW_MGMT_TOKEN_ENDPOINT` | -- | Custom token endpoint |
| `PANW_DLP_ENDPOINT` | -- | Custom DLP API base URL (default: `api.dlp.paloaltonetworks.com`) — used by `airs runtime dlp` |

---

## AIRS Red Team API

| Variable | Required | What it does |
|----------|:--------:|-------------|
| `PANW_RED_TEAM_DATA_ENDPOINT` | -- | Custom Red Team data-plane endpoint |
| `PANW_RED_TEAM_MGMT_ENDPOINT` | -- | Custom Red Team management-plane endpoint |
| `PANW_RED_TEAM_TOKEN_ENDPOINT` | -- | Custom Red Team OAuth2 token endpoint |

:::note
Red Team commands reuse `PANW_MGMT_CLIENT_ID`, `PANW_MGMT_CLIENT_SECRET`, and `PANW_MGMT_TSG_ID` from the Management API section. The variables above are optional overrides for dedicated Red Team endpoints.
:::

---

## AIRS Model Security API

| Variable | Required | What it does |
|----------|:--------:|-------------|
| `PANW_MODEL_SEC_DATA_ENDPOINT` | -- | Custom Model Security data-plane endpoint |
| `PANW_MODEL_SEC_MGMT_ENDPOINT` | -- | Custom Model Security management-plane endpoint |
| `PANW_MODEL_SEC_TOKEN_ENDPOINT` | -- | Custom Model Security OAuth2 token endpoint |

:::note
Model Security commands also reuse the `PANW_MGMT_*` credentials. All endpoint overrides above (and their Red Team counterparts) can equally be set in `~/.prisma-airs/config.json` via the matching config keys (`airsApiToken`, `airsApiEndpoint`, `airsNumRetries`, `redTeamDataEndpoint`, `redTeamMgmtEndpoint`, `redTeamTokenEndpoint`, `modelSecDataEndpoint`, `modelSecMgmtEndpoint`, `modelSecTokenEndpoint`). Precedence: CLI > environment > config file.
:::

---

## Tuning

| Variable | Default | Range | What it controls |
|----------|---------|-------|-----------------|
| `SCAN_CONCURRENCY` | `5` | 1--20 | Parallel scan requests per batch |

:::warning
`SCAN_CONCURRENCY` above 5 may trigger AIRS rate limits. Increase cautiously.
:::

---

## Paths

| Variable | Default | What it does |
|----------|---------|-------------|
| `DATA_DIR` | `~/.prisma-airs/runs` | Data directory |

:::tip
The `~` prefix is expanded to `$HOME` automatically.
:::
