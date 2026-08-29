# Configuration Options

Every setting in Prisma AIRS CLI — with its CLI flag, env var, and default value.

---

## Config Cascade

Settings resolve through a four-level cascade (highest priority wins):

1. **CLI flags**
2. **Environment variables** (`SCAN_CONCURRENCY`, `DATA_DIR`, etc.)
3. **Config file** (`~/.prisma-airs/config.json`)
4. **Zod schema defaults**

:::info
The `~` prefix in any path value is expanded to `$HOME` at load time.
:::

---

## Config File

Optional JSON file at `~/.prisma-airs/config.json`. Keys use camelCase matching the Zod schema.

```json
{
  "scanConcurrency": 3,
  "dataDir": "~/.prisma-airs/runs",
  "defaultOutput": "json"
}
```

---

## General settings

| Setting | CLI Flag | Env Var | Default | What it does |
|---------|----------|---------|---------|-------------|
| `scanConcurrency` | -- | `SCAN_CONCURRENCY` | `5` | Parallel scan requests (1--20) |
| `dataDir` | -- | `DATA_DIR` | `~/.prisma-airs/runs` | Data directory |
| `defaultOutput` | global `--output` | `PANW_CLI_OUTPUT` | `pretty` | Default read format (`pretty`, `table`, `markdown`, `csv`, `json`, `yaml`) |

## Credentials and endpoint overrides

| Config key | Environment variable | Purpose |
|------------|----------------------|---------|
| `airsApiKey` | `PANW_AI_SEC_API_KEY` | Scan API key |
| `airsApiToken` | `PANW_AI_SEC_API_TOKEN` | Pre-obtained scan bearer token |
| `airsApiEndpoint` | `PANW_AI_SEC_API_ENDPOINT` | Scan API endpoint |
| `airsNumRetries` | `PANW_AI_SEC_NUM_RETRIES` | SDK retry count, 0--5 |
| `mgmtClientId` | `PANW_MGMT_CLIENT_ID` | Shared OAuth client ID |
| `mgmtClientSecret` | `PANW_MGMT_CLIENT_SECRET` | Shared OAuth client secret |
| `mgmtTsgId` | `PANW_MGMT_TSG_ID` | Tenant service group ID |
| `mgmtEndpoint` | `PANW_MGMT_ENDPOINT` | Management API endpoint |
| `mgmtTokenEndpoint` | `PANW_MGMT_TOKEN_ENDPOINT` | Management OAuth endpoint |
| `dlpEndpoint` | `PANW_DLP_ENDPOINT` | DLP API endpoint |
| `redTeamDataEndpoint` | `PANW_RED_TEAM_DATA_ENDPOINT` | Red Team data-plane endpoint |
| `redTeamMgmtEndpoint` | `PANW_RED_TEAM_MGMT_ENDPOINT` | Red Team management endpoint |
| `redTeamTokenEndpoint` | `PANW_RED_TEAM_TOKEN_ENDPOINT` | Red Team OAuth endpoint |
| `redTeamNetworkBrokerEndpoint` | `PANW_RED_TEAM_NETWORK_BROKER_ENDPOINT` | Network broker endpoint |
| `modelSecDataEndpoint` | `PANW_MODEL_SEC_DATA_ENDPOINT` | Model Security data endpoint |
| `modelSecMgmtEndpoint` | `PANW_MODEL_SEC_MGMT_ENDPOINT` | Model Security management endpoint |
| `modelSecTokenEndpoint` | `PANW_MODEL_SEC_TOKEN_ENDPOINT` | Model Security OAuth endpoint |
| `aiGwDataEndpoint` | `PANW_AI_GW_DATA_ENDPOINT` | AI Gateway data-plane endpoint |
| `aiGwAdminEndpoint` | `PANW_AI_GW_ADMIN_ENDPOINT` | AI Gateway admin endpoint |
| `aiGwTokenEndpoint` | `PANW_AI_GW_TOKEN_ENDPOINT` | AI Gateway OAuth endpoint |

The config file location itself is overridden with
`PRISMA_AIRS_CONFIG_PATH`; it is not a `ConfigSchema` key.

:::warning[Concurrency tuning]
`scanConcurrency` above 5 risks AIRS rate limiting. Increase cautiously.
:::
