# Function: loadConfig()

> **loadConfig**(`cliOverrides?`, `configFilePath?`): `Promise`\<\{ `airsApiKey?`: `string`; `anthropicApiKey?`: `string`; `awsAccessKeyId?`: `string`; `awsRegion`: `string`; `awsSecretAccessKey?`: `string`; `dataDir`: `string`; `dlpEndpoint?`: `string`; `googleApiKey?`: `string`; `googleCloudLocation`: `string`; `googleCloudProject?`: `string`; `llmModel?`: `string`; `llmProvider`: `"claude-api"` \| `"claude-vertex"` \| `"claude-bedrock"` \| `"gemini-api"` \| `"gemini-vertex"` \| `"gemini-bedrock"`; `mgmtClientId?`: `string`; `mgmtClientSecret?`: `string`; `mgmtEndpoint?`: `string`; `mgmtTokenEndpoint?`: `string`; `mgmtTsgId?`: `string`; `scanConcurrency`: `number`; \}\>

Defined in: [src/config/loader.ts:47](https://github.com/cdot65/prisma-airs-cli/blob/main/src/config/loader.ts#L47)

## Parameters

### cliOverrides?

`Record`\<`string`, `unknown`\> = `{}`

### configFilePath?

`string`

## Returns

`Promise`\<\{ `airsApiKey?`: `string`; `anthropicApiKey?`: `string`; `awsAccessKeyId?`: `string`; `awsRegion`: `string`; `awsSecretAccessKey?`: `string`; `dataDir`: `string`; `dlpEndpoint?`: `string`; `googleApiKey?`: `string`; `googleCloudLocation`: `string`; `googleCloudProject?`: `string`; `llmModel?`: `string`; `llmProvider`: `"claude-api"` \| `"claude-vertex"` \| `"claude-bedrock"` \| `"gemini-api"` \| `"gemini-vertex"` \| `"gemini-bedrock"`; `mgmtClientId?`: `string`; `mgmtClientSecret?`: `string`; `mgmtEndpoint?`: `string`; `mgmtTokenEndpoint?`: `string`; `mgmtTsgId?`: `string`; `scanConcurrency`: `number`; \}\>
