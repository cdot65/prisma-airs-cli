# Function: loadConfig()

> **loadConfig**(`cliOverrides?`, `configFilePath?`): `Promise`\<\{ `aiGwAdminEndpoint?`: `string`; `aiGwDataEndpoint?`: `string`; `aiGwTokenEndpoint?`: `string`; `airsApiEndpoint?`: `string`; `airsApiKey?`: `string`; `airsApiToken?`: `string`; `airsNumRetries?`: `number`; `dataDir`: `string`; `defaultOutput?`: `"json"` \| `"yaml"` \| `"pretty"` \| `"table"` \| `"markdown"` \| `"csv"`; `dlpEndpoint?`: `string`; `mgmtClientId?`: `string`; `mgmtClientSecret?`: `string`; `mgmtEndpoint?`: `string`; `mgmtTokenEndpoint?`: `string`; `mgmtTsgId?`: `string`; `modelSecDataEndpoint?`: `string`; `modelSecMgmtEndpoint?`: `string`; `modelSecTokenEndpoint?`: `string`; `redTeamDataEndpoint?`: `string`; `redTeamMgmtEndpoint?`: `string`; `redTeamNetworkBrokerEndpoint?`: `string`; `redTeamTokenEndpoint?`: `string`; `scanConcurrency`: `number`; \}\>

Defined in: src/config/loader.ts:63

## Parameters

### cliOverrides?

`Record`\<`string`, `unknown`\> = `{}`

### configFilePath?

`string`

## Returns

`Promise`\<\{ `aiGwAdminEndpoint?`: `string`; `aiGwDataEndpoint?`: `string`; `aiGwTokenEndpoint?`: `string`; `airsApiEndpoint?`: `string`; `airsApiKey?`: `string`; `airsApiToken?`: `string`; `airsNumRetries?`: `number`; `dataDir`: `string`; `defaultOutput?`: `"json"` \| `"yaml"` \| `"pretty"` \| `"table"` \| `"markdown"` \| `"csv"`; `dlpEndpoint?`: `string`; `mgmtClientId?`: `string`; `mgmtClientSecret?`: `string`; `mgmtEndpoint?`: `string`; `mgmtTokenEndpoint?`: `string`; `mgmtTsgId?`: `string`; `modelSecDataEndpoint?`: `string`; `modelSecMgmtEndpoint?`: `string`; `modelSecTokenEndpoint?`: `string`; `redTeamDataEndpoint?`: `string`; `redTeamMgmtEndpoint?`: `string`; `redTeamNetworkBrokerEndpoint?`: `string`; `redTeamTokenEndpoint?`: `string`; `scanConcurrency`: `number`; \}\>
