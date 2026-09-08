# Function: loadConfig()

> **loadConfig**(`cliOverrides?`, `configFilePath?`): `Promise`\<\{ `agentGuardDataEndpoint?`: `string`; `agentGuardMgmtEndpoint?`: `string`; `agentGuardTokenEndpoint?`: `string`; `aiGwAdminEndpoint?`: `string`; `aiGwDataEndpoint?`: `string`; `aiGwEmbeddingModel?`: `string`; `aiGwInferenceApiKey?`: `string`; `aiGwInferenceEndpoint?`: `string`; `aiGwInferenceModel?`: `string`; `aiGwTokenEndpoint?`: `string`; `airsApiEndpoint?`: `string`; `airsApiKey?`: `string`; `airsApiToken?`: `string`; `airsNumRetries?`: `number`; `dataDir`: `string`; `defaultOutput?`: `"json"` \| `"yaml"` \| `"pretty"` \| `"table"` \| `"markdown"` \| `"csv"`; `dlpEndpoint?`: `string`; `mgmtClientId?`: `string`; `mgmtClientSecret?`: `string`; `mgmtDashboardEndpoint?`: `string`; `mgmtEndpoint?`: `string`; `mgmtTokenEndpoint?`: `string`; `mgmtTsgId?`: `string`; `modelSecDataEndpoint?`: `string`; `modelSecMgmtEndpoint?`: `string`; `modelSecTokenEndpoint?`: `string`; `redTeamDataEndpoint?`: `string`; `redTeamMgmtEndpoint?`: `string`; `redTeamNetworkBrokerEndpoint?`: `string`; `redTeamTokenEndpoint?`: `string`; `scanConcurrency`: `number`; \}\>

Defined in: src/config/loader.ts:84

## Parameters

### cliOverrides?

`Record`\<`string`, `unknown`\> = `{}`

### configFilePath?

`string`

## Returns

`Promise`\<\{ `agentGuardDataEndpoint?`: `string`; `agentGuardMgmtEndpoint?`: `string`; `agentGuardTokenEndpoint?`: `string`; `aiGwAdminEndpoint?`: `string`; `aiGwDataEndpoint?`: `string`; `aiGwEmbeddingModel?`: `string`; `aiGwInferenceApiKey?`: `string`; `aiGwInferenceEndpoint?`: `string`; `aiGwInferenceModel?`: `string`; `aiGwTokenEndpoint?`: `string`; `airsApiEndpoint?`: `string`; `airsApiKey?`: `string`; `airsApiToken?`: `string`; `airsNumRetries?`: `number`; `dataDir`: `string`; `defaultOutput?`: `"json"` \| `"yaml"` \| `"pretty"` \| `"table"` \| `"markdown"` \| `"csv"`; `dlpEndpoint?`: `string`; `mgmtClientId?`: `string`; `mgmtClientSecret?`: `string`; `mgmtDashboardEndpoint?`: `string`; `mgmtEndpoint?`: `string`; `mgmtTokenEndpoint?`: `string`; `mgmtTsgId?`: `string`; `modelSecDataEndpoint?`: `string`; `modelSecMgmtEndpoint?`: `string`; `modelSecTokenEndpoint?`: `string`; `redTeamDataEndpoint?`: `string`; `redTeamMgmtEndpoint?`: `string`; `redTeamNetworkBrokerEndpoint?`: `string`; `redTeamTokenEndpoint?`: `string`; `scanConcurrency`: `number`; \}\>
