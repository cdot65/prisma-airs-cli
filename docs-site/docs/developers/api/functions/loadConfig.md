# Function: loadConfig()

> **loadConfig**(`cliOverrides?`, `configFilePath?`): `Promise`\<\{ `airsApiEndpoint?`: `string`; `airsApiKey?`: `string`; `airsApiToken?`: `string`; `airsNumRetries?`: `number`; `dataDir`: `string`; `dlpEndpoint?`: `string`; `mgmtClientId?`: `string`; `mgmtClientSecret?`: `string`; `mgmtEndpoint?`: `string`; `mgmtTokenEndpoint?`: `string`; `mgmtTsgId?`: `string`; `modelSecDataEndpoint?`: `string`; `modelSecMgmtEndpoint?`: `string`; `modelSecTokenEndpoint?`: `string`; `redTeamDataEndpoint?`: `string`; `redTeamMgmtEndpoint?`: `string`; `redTeamNetworkBrokerEndpoint?`: `string`; `redTeamTokenEndpoint?`: `string`; `scanConcurrency`: `number`; \}\>

Defined in: [src/config/loader.ts:59](https://github.com/cdot65/prisma-airs-cli/blob/main/src/config/loader.ts#L59)

## Parameters

### cliOverrides?

`Record`\<`string`, `unknown`\> = `{}`

### configFilePath?

`string`

## Returns

`Promise`\<\{ `airsApiEndpoint?`: `string`; `airsApiKey?`: `string`; `airsApiToken?`: `string`; `airsNumRetries?`: `number`; `dataDir`: `string`; `dlpEndpoint?`: `string`; `mgmtClientId?`: `string`; `mgmtClientSecret?`: `string`; `mgmtEndpoint?`: `string`; `mgmtTokenEndpoint?`: `string`; `mgmtTsgId?`: `string`; `modelSecDataEndpoint?`: `string`; `modelSecMgmtEndpoint?`: `string`; `modelSecTokenEndpoint?`: `string`; `redTeamDataEndpoint?`: `string`; `redTeamMgmtEndpoint?`: `string`; `redTeamNetworkBrokerEndpoint?`: `string`; `redTeamTokenEndpoint?`: `string`; `scanConcurrency`: `number`; \}\>
