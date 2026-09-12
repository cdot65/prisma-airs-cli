# Configuration Options

Every setting in Prisma AIRS CLI, with its tenant-file key and default value.

---

## Config Cascade

Settings resolve through a three-level cascade (highest priority wins):

1. **CLI flags**
2. **The selected tenant's config file** (`airs tenant switch <name>`)
3. **Zod schema defaults**

Environment variables are never consulted. See
[environment variables](./environment-variables.md) for the two registry-location variables
and the SDK diagnostic switches.

:::info
The `~` prefix in any path value is expanded to `$HOME` at load time.
:::

---

## Tenant config file

A JSON file registered with `airs tenant create`. Keys use camelCase matching the Zod schema.
Edit keys with `airs tenant set <name> <key> [value]`, remove them with
`airs tenant unset <name> <key>`, and read them with `airs tenant get` or `airs tenant read`.

```json
{
  "mgmtClientId": "airs-cli@1234567890.iam.panserviceaccount.com",
  "mgmtClientSecret": "…",
  "mgmtTsgId": "1234567890",
  "airsApiKey": "…",
  "scanConcurrency": 3,
  "defaultOutput": "json"
}
```

---

## Credentials

There is exactly one OAuth credential set per tenant. Management, DLP, Red Team, Model Security,
AgentGuard, AI Gateway, and SCM IAM commands all authenticate with it, through one token endpoint.

| Key | Required | Purpose |
|-----|:--------:|---------|
| `mgmtClientId` | Yes | SCM OAuth2 client ID |
| `mgmtClientSecret` | Yes | SCM OAuth2 client secret |
| `mgmtTsgId` | Yes | Tenant service group ID; pinned at registration |
| `mgmtTokenEndpoint` | -- | OAuth2 token URL for every product (default `https://auth.apps.paloaltonetworks.com/oauth2/access_token`) |
| `airsApiKey` | scan commands | Runtime scan API key |
| `airsApiToken` | scan commands | Pre-obtained scan bearer token (alternative to the key) |
| `aiGwInferenceEndpoint`, `aiGwInferenceApiKey` | inference commands | AI Gateway runtime base URL and workspace key; not SCM OAuth |
| `aiGwInferenceModel`, `aiGwEmbeddingModel` | -- | Default models for inference commands |

## Endpoints

Base URLs default to the SDK constants. Overrides exist for test servers and private routing;
they never change how authentication works.

| Key | Default |
|-----|---------|
| `airsApiEndpoint` | `https://service.api.aisecurity.paloaltonetworks.com` |
| `mgmtEndpoint` | `https://api.apps.paloaltonetworks.com/aisec` |
| `mgmtDashboardEndpoint` | `https://api.apps.paloaltonetworks.com/aisec` |
| `dlpEndpoint` | `https://api.dlp.paloaltonetworks.com` |
| `redTeamDataEndpoint`, `redTeamMgmtEndpoint`, `redTeamNetworkBrokerEndpoint` | `https://api.apps.paloaltonetworks.com/ai-red-teaming/{data-plane,mgmt-plane,data-plane/network-broker}` |
| `modelSecDataEndpoint`, `modelSecMgmtEndpoint` | `https://api.apps.paloaltonetworks.com/aims/{data,mgmt}` |
| `agentGuardDataEndpoint`, `agentGuardMgmtEndpoint` | `https://api.apps.paloaltonetworks.com/aiag/{data,mgmt}` |
| `aiGwDataEndpoint`, `aiGwAdminEndpoint` | `https://api.apps.paloaltonetworks.com/ai_gw/v2`, `…/ai_gw/admin/v2` |
| `iamEndpoint` | `https://api.apps.paloaltonetworks.com/iam/v1` |

Every product except DLP lives on `api.apps.paloaltonetworks.com`; DLP stays on `api.dlp.paloaltonetworks.com`. Retired keys `redTeamTokenEndpoint`, `modelSecTokenEndpoint`, `agentGuardTokenEndpoint`, and
`aiGwTokenEndpoint` are ignored when present; `airs doctor` points them out.

## Tuning

| Key | Default | What it does |
|-----|---------|--------------|
| `scanConcurrency` | `5` | Parallel scan requests (1--20) |
| `airsNumRetries` | `5` | SDK retry count for scan calls (0--5) |
| `defaultOutput` | `pretty` | Default read format (`pretty`, `table`, `markdown`, `csv`, `json`, `yaml`); the global `--output` flag overrides it |
| `dataDir` | `~/.prisma-airs/runs` | Bulk-scan state directory |

:::warning[Concurrency tuning]
`scanConcurrency` above 5 risks AIRS rate limiting. Increase cautiously.
:::
