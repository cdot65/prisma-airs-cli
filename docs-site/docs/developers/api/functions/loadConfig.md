# Function: loadConfig()

> **loadConfig**(`cliOverrides?`, `configFilePath?`): `Promise`\<\{ `agentGuardDataEndpoint?`: `string`; `agentGuardMgmtEndpoint?`: `string`; `aiGwAdminEndpoint?`: `string`; `aiGwDataEndpoint?`: `string`; `aiGwEmbeddingModel?`: `string`; `aiGwInferenceApiKey?`: `string`; `aiGwInferenceEndpoint?`: `string`; `aiGwInferenceModel?`: `string`; `airsApiEndpoint?`: `string`; `airsApiKey?`: `string`; `airsApiToken?`: `string`; `airsNumRetries?`: `number`; `dataDir`: `string`; `defaultOutput?`: `"json"` \| `"yaml"` \| `"pretty"` \| `"table"` \| `"markdown"` \| `"csv"`; `dlpEndpoint?`: `string`; `iamEndpoint?`: `string`; `mgmtClientId?`: `string`; `mgmtClientSecret?`: `string`; `mgmtDashboardEndpoint?`: `string`; `mgmtEndpoint?`: `string`; `mgmtTokenEndpoint?`: `string`; `mgmtTsgId?`: `string`; `modelSecDataEndpoint?`: `string`; `modelSecMgmtEndpoint?`: `string`; `redTeamDataEndpoint?`: `string`; `redTeamMgmtEndpoint?`: `string`; `redTeamNetworkBrokerEndpoint?`: `string`; `scanConcurrency`: `number`; \}\>

Defined in: src/config/loader.ts:84

Load the effective config: CLI overrides > selected tenant file > defaults.
The environment is never consulted.

## Parameters

### cliOverrides?

`Record`\<`string`, `unknown`\> = `{}`

### configFilePath?

`string`

## Returns

`Promise`\<\{ `agentGuardDataEndpoint?`: `string`; `agentGuardMgmtEndpoint?`: `string`; `aiGwAdminEndpoint?`: `string`; `aiGwDataEndpoint?`: `string`; `aiGwEmbeddingModel?`: `string`; `aiGwInferenceApiKey?`: `string`; `aiGwInferenceEndpoint?`: `string`; `aiGwInferenceModel?`: `string`; `airsApiEndpoint?`: `string`; `airsApiKey?`: `string`; `airsApiToken?`: `string`; `airsNumRetries?`: `number`; `dataDir`: `string`; `defaultOutput?`: `"json"` \| `"yaml"` \| `"pretty"` \| `"table"` \| `"markdown"` \| `"csv"`; `dlpEndpoint?`: `string`; `iamEndpoint?`: `string`; `mgmtClientId?`: `string`; `mgmtClientSecret?`: `string`; `mgmtDashboardEndpoint?`: `string`; `mgmtEndpoint?`: `string`; `mgmtTokenEndpoint?`: `string`; `mgmtTsgId?`: `string`; `modelSecDataEndpoint?`: `string`; `modelSecMgmtEndpoint?`: `string`; `redTeamDataEndpoint?`: `string`; `redTeamMgmtEndpoint?`: `string`; `redTeamNetworkBrokerEndpoint?`: `string`; `scanConcurrency`: `number`; \}\>
