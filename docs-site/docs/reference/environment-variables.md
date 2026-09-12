# Environment Variables

Prisma AIRS CLI reads **no configuration from the environment**. Credentials, endpoints, output
defaults, and tuning all live in the selected tenant's config file, managed with
[`airs tenant`](../cli/tenant.md). Every `PANW_*` variable from earlier releases, the
`PRISMA_AIRS_CONFIG_PATH` override, and a `.env` file in the working directory are ignored; run
`airs doctor` to list any that are still set in your shell.

## Registry location

These two variables choose **where the tenant registry lives**. They never supply a
configuration value.

| Variable | Default | Purpose |
| --- | --- | --- |
| `PRISMA_AIRS_TENANTS_PATH` | `$XDG_STATE_HOME/prisma-airs/tenants.json` | Separate registry for a container, automation job, or shell session |
| `XDG_STATE_HOME` | `~/.local/state` | Absolute state directory used when no registry override is set |

Use a private registry to run parallel jobs against different tenants: each job gets its own
`tenants.json` selecting the tenant it should use.

## SDK diagnostics

The SDK reads these on every request. The CLI honors them as-is; `airs doctor` mentions when
they are on.

| Variable | Default | What it does |
|----------|---------|-------------|
| `PANW_AI_SEC_DEBUG` | off | Redacted request/response logging from the SDK; prefer `airs --debug`, which writes a private JSONL file |
| `PANW_AI_SEC_DEBUG_BODY` | off | Also log bodies; the CLI forces it off for DLP and AgentGuard commands |
| `PANW_AI_SEC_TIMEOUT_MS` | `60000` | Per-request HTTP timeout in the SDK |

Profile transfer and environment reports refuse to run while `PANW_AI_SEC_DEBUG` or `--debug` is on.

## Ignored names

Setting any of these does nothing. `airs doctor` warns so stale shell profiles get cleaned up.

| Name | Where the setting lives now |
|------|-----------------------------|
| `PANW_AI_SEC_API_KEY`, `PANW_AI_SEC_API_TOKEN`, `PANW_AI_SEC_API_ENDPOINT`, `PANW_AI_SEC_NUM_RETRIES` | `airsApiKey`, `airsApiToken`, `airsApiEndpoint`, `airsNumRetries` in the tenant file |
| `PANW_MGMT_CLIENT_ID`, `PANW_MGMT_CLIENT_SECRET`, `PANW_MGMT_TSG_ID` | `airs tenant create <name>` prompts for them; rotate with `airs tenant set <name> mgmtClientSecret` |
| `PANW_MGMT_ENDPOINT`, `PANW_MGMT_TOKEN_ENDPOINT`, `PANW_MGMT_DASHBOARD_ENDPOINT` | `mgmtEndpoint`, `mgmtTokenEndpoint`, `mgmtDashboardEndpoint` |
| `PANW_DLP_ENDPOINT`, `PANW_RED_TEAM_*_ENDPOINT`, `PANW_MODEL_SEC_*_ENDPOINT`, `PANW_AGENT_GUARD_*_ENDPOINT`, `PANW_AI_GW_*_ENDPOINT`, `PANW_IAM_ENDPOINT` | the matching base-URL keys listed in [configuration options](./configuration.md); per-product token endpoints no longer exist |
| `PANW_RED_TEAM_CLIENT_ID`, `PANW_MODEL_SEC_CLIENT_ID`, `PANW_AI_GW_CLIENT_ID`, `PANW_AGENT_GUARD_CLIENT_ID` and their `_CLIENT_SECRET` / `_TSG_ID` | nowhere: every product authenticates with the tenant's single `mgmt*` credential set |
| `PANW_AI_GW_INFERENCE_*` | `aiGwInferenceEndpoint`, `aiGwInferenceApiKey`, `aiGwInferenceModel`, `aiGwEmbeddingModel` |
| `PANW_CLI_OUTPUT`, `PANW_CLI_SCAN_CONCURRENCY`, `PANW_CLI_DATA_DIR`, `SCAN_CONCURRENCY`, `DATA_DIR` | `defaultOutput`, `scanConcurrency`, `dataDir` |
| `PRISMA_AIRS_CONFIG_PATH` | `airs tenant switch <name>`, or a private registry via `PRISMA_AIRS_TENANTS_PATH` |
| `MEMORY_*`, `ACCUMULATE_TESTS`, `MAX_ACCUMULATED_TESTS` | removed features |

The per-product credential names remain an SDK feature for library users. The CLI passes every
credential and endpoint to the SDK explicitly, so the SDK never falls back to them either.
