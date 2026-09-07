# Interface: ManagementService

Defined in: prisma-airs-cli/src/airs/types.ts:1241

Contract for AIRS topic CRUD, profile CRUD, and profile linking operations.

## Methods

### assignTopicsToProfile()

> **assignTopicsToProfile**(`profileName`, `topics`, `guardrailAction?`): `Promise`\<`void`\>

Defined in: prisma-airs-cli/src/airs/types.ts:1266

Assign multiple topics to a security profile's topic-guardrails.

#### Parameters

##### profileName

`string`

##### topics

`object`[]

##### guardrailAction?

`"allow"` \| `"block"`

#### Returns

`Promise`\<`void`\>

***

### assignTopicToProfile()

> **assignTopicToProfile**(`profileName`, `topicId`, `topicName`, `action`): `Promise`\<`void`\>

Defined in: prisma-airs-cli/src/airs/types.ts:1259

Assign a topic to a security profile's topic-guardrails.

#### Parameters

##### profileName

`string`

##### topicId

`string`

##### topicName

`string`

##### action

`"allow"` \| `"block"`

#### Returns

`Promise`\<`void`\>

***

### createApiKey()

> **createApiKey**(`request`): `Promise`\<[`ApiKeyInfo`](ApiKeyInfo.md)\>

Defined in: prisma-airs-cli/src/airs/types.ts:1299

#### Parameters

##### request

`Record`\<`string`, `unknown`\>

#### Returns

`Promise`\<[`ApiKeyInfo`](ApiKeyInfo.md)\>

***

### createProfile()

> **createProfile**(`request`): `Promise`\<[`SecurityProfileInfo`](SecurityProfileInfo.md)\>

Defined in: prisma-airs-cli/src/airs/types.ts:1285

Create a security profile.

#### Parameters

##### request

`CreateSecurityProfileRequest`

#### Returns

`Promise`\<[`SecurityProfileInfo`](SecurityProfileInfo.md)\>

***

### createTopic()

> **createTopic**(`request`): `Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodDefault`\<`ZodArray`\<`ZodString`, `"many"`\>\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>\>

Defined in: prisma-airs-cli/src/airs/types.ts:1243

Create a new custom topic.

#### Parameters

##### request

`objectOutputType`

#### Returns

`Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodDefault`\<`ZodArray`\<`ZodString`, `"many"`\>\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>\>

***

### deleteApiKey()

> **deleteApiKey**(`apiKeyName`, `updatedBy`): `Promise`\<[`DeleteResponse`](DeleteResponse.md)\>

Defined in: prisma-airs-cli/src/airs/types.ts:1301

#### Parameters

##### apiKeyName

`string`

##### updatedBy

`string`

#### Returns

`Promise`\<[`DeleteResponse`](DeleteResponse.md)\>

***

### deleteCustomerApp()

> **deleteCustomerApp**(`appName`, `updatedBy`): `Promise`\<[`CustomerAppInfo`](CustomerAppInfo.md)\>

Defined in: prisma-airs-cli/src/airs/types.ts:1308

#### Parameters

##### appName

`string`

##### updatedBy

`string`

#### Returns

`Promise`\<[`CustomerAppInfo`](CustomerAppInfo.md)\>

***

### deleteProfile()

> **deleteProfile**(`profileId`): `Promise`\<[`DeleteResponse`](DeleteResponse.md)\>

Defined in: prisma-airs-cli/src/airs/types.ts:1292

Delete a security profile.

#### Parameters

##### profileId

`string`

#### Returns

`Promise`\<[`DeleteResponse`](DeleteResponse.md)\>

***

### deleteTopic()

> **deleteTopic**(`topicId`): `Promise`\<`void`\>

Defined in: prisma-airs-cli/src/airs/types.ts:1247

Delete a custom topic by ID.

#### Parameters

##### topicId

`string`

#### Returns

`Promise`\<`void`\>

***

### forceDeleteProfile()

> **forceDeleteProfile**(`profileId`, `updatedBy`): `Promise`\<[`DeleteResponse`](DeleteResponse.md)\>

Defined in: prisma-airs-cli/src/airs/types.ts:1294

Force-delete a security profile (removes from referencing policies).

#### Parameters

##### profileId

`string`

##### updatedBy

`string`

#### Returns

`Promise`\<[`DeleteResponse`](DeleteResponse.md)\>

***

### forceDeleteTopic()

> **forceDeleteTopic**(`topicId`, `updatedBy?`): `Promise`\<[`DeleteResponse`](DeleteResponse.md)\>

Defined in: prisma-airs-cli/src/airs/types.ts:1249

Force-delete a custom topic (removes from all referencing profiles).

#### Parameters

##### topicId

`string`

##### updatedBy?

`string`

#### Returns

`Promise`\<[`DeleteResponse`](DeleteResponse.md)\>

***

### getCustomerApp()

> **getCustomerApp**(`appName`): `Promise`\<[`CustomerAppInfo`](CustomerAppInfo.md)\>

Defined in: prisma-airs-cli/src/airs/types.ts:1306

#### Parameters

##### appName

`string`

#### Returns

`Promise`\<[`CustomerAppInfo`](CustomerAppInfo.md)\>

***

### getCustomerAppConsumption()

> **getCustomerAppConsumption**(`appName`, `opts?`): `Promise`\<[`CustomerAppConsumption`](CustomerAppConsumption.md)\>

Defined in: prisma-airs-cli/src/airs/types.ts:1310

Get per-app token consumption + violation breakdown from the SCM dashboard endpoints.

#### Parameters

##### appName

`string`

##### opts?

[`ConsumptionQueryOptions`](ConsumptionQueryOptions.md)

#### Returns

`Promise`\<[`CustomerAppConsumption`](CustomerAppConsumption.md)\>

***

### getProfile()

> **getProfile**(`profileId`): `Promise`\<[`SecurityProfileInfo`](SecurityProfileInfo.md)\>

Defined in: prisma-airs-cli/src/airs/types.ts:1275

Get a single security profile by UUID.

#### Parameters

##### profileId

`string`

#### Returns

`Promise`\<[`SecurityProfileInfo`](SecurityProfileInfo.md)\>

***

### getProfileByName()

> **getProfileByName**(`profileName`): `Promise`\<[`SecurityProfileInfo`](SecurityProfileInfo.md)\>

Defined in: prisma-airs-cli/src/airs/types.ts:1277

Get a single security profile by name (returns highest revision).

#### Parameters

##### profileName

`string`

#### Returns

`Promise`\<[`SecurityProfileInfo`](SecurityProfileInfo.md)\>

***

### getProfileTopics()

> **getProfileTopics**(`profileName`): `Promise`\<[`ProfileTopic`](ProfileTopic.md)[]\>

Defined in: prisma-airs-cli/src/airs/types.ts:1272

List all topics configured in a profile with full details.

#### Parameters

##### profileName

`string`

#### Returns

`Promise`\<[`ProfileTopic`](ProfileTopic.md)[]\>

***

### getTopic()

> **getTopic**(`topicId`): `Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodDefault`\<`ZodArray`\<`ZodString`, `"many"`\>\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>\>

Defined in: prisma-airs-cli/src/airs/types.ts:1255

Get a single custom topic by ID.

#### Parameters

##### topicId

`string`

#### Returns

`Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodDefault`\<`ZodArray`\<`ZodString`, `"many"`\>\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>\>

***

### getTopicByName()

> **getTopicByName**(`topicName`): `Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodDefault`\<`ZodArray`\<`ZodString`, `"many"`\>\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>\>

Defined in: prisma-airs-cli/src/airs/types.ts:1257

Get a single custom topic by name.

#### Parameters

##### topicName

`string`

#### Returns

`Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodDefault`\<`ZodArray`\<`ZodString`, `"many"`\>\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>\>

***

### listAllApiKeys()

> **listAllApiKeys**(`opts?`): `Promise`\<[`ApiKeyInfo`](ApiKeyInfo.md)[]\>

Defined in: prisma-airs-cli/src/airs/types.ts:1298

#### Parameters

##### opts?

###### limit?

`number`

###### max?

`number`

#### Returns

`Promise`\<[`ApiKeyInfo`](ApiKeyInfo.md)[]\>

***

### listAllCustomerApps()

> **listAllCustomerApps**(`opts?`): `Promise`\<[`CustomerAppInfo`](CustomerAppInfo.md)[]\>

Defined in: prisma-airs-cli/src/airs/types.ts:1305

#### Parameters

##### opts?

###### limit?

`number`

###### max?

`number`

#### Returns

`Promise`\<[`CustomerAppInfo`](CustomerAppInfo.md)[]\>

***

### listAllProfiles()

> **listAllProfiles**(`opts?`): `Promise`\<[`SecurityProfileInfo`](SecurityProfileInfo.md)[]\>

Defined in: prisma-airs-cli/src/airs/types.ts:1281

Walk all security-profile pages.

#### Parameters

##### opts?

`Omit`\<[`PaginationOptions`](PaginationOptions.md), `"offset"`\> & `object`

#### Returns

`Promise`\<[`SecurityProfileInfo`](SecurityProfileInfo.md)[]\>

***

### listApiKeys()

> **listApiKeys**(`opts?`): `Promise`\<[`ApiKeyListResult`](ApiKeyListResult.md)\>

Defined in: prisma-airs-cli/src/airs/types.ts:1297

#### Parameters

##### opts?

[`PaginationOptions`](PaginationOptions.md)

#### Returns

`Promise`\<[`ApiKeyListResult`](ApiKeyListResult.md)\>

***

### listConsumptionApps()

> **listConsumptionApps**(`opts?`): `Promise`\<[`ConsumptionAppListEntry`](ConsumptionAppListEntry.md)[]\>

Defined in: prisma-airs-cli/src/airs/types.ts:1322

List dashboard application buckets - the canonical apps source for consumption reporting.

Drawn from `dashboard.applicationsOverview`. One entry per dashboard bucket, which is one
per distinct scan-payload `metadata.app_name` per registered customer-app. Distinct from
[ManagementService.listCustomerApps](#listcustomerapps), which enumerates registered customer-apps
(different granularity).

#### Parameters

##### opts?

###### limit?

`number`

###### offset?

`number`

#### Returns

`Promise`\<[`ConsumptionAppListEntry`](ConsumptionAppListEntry.md)[]\>

***

### listCustomerApps()

> **listCustomerApps**(`opts?`): `Promise`\<[`CustomerAppListResult`](CustomerAppListResult.md)\>

Defined in: prisma-airs-cli/src/airs/types.ts:1304

#### Parameters

##### opts?

[`PaginationOptions`](PaginationOptions.md)

#### Returns

`Promise`\<[`CustomerAppListResult`](CustomerAppListResult.md)\>

***

### listDeploymentProfiles()

> **listDeploymentProfiles**(`opts?`): `Promise`\<[`DeploymentProfileInfo`](DeploymentProfileInfo.md)[]\>

Defined in: prisma-airs-cli/src/airs/types.ts:1328

#### Parameters

##### opts?

###### unactivated?

`boolean`

#### Returns

`Promise`\<[`DeploymentProfileInfo`](DeploymentProfileInfo.md)[]\>

***

### listLatestTopics()

> **listLatestTopics**(`opts?`): `Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodDefault`\<`ZodArray`\<`ZodString`, `"many"`\>\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>[]\>

Defined in: prisma-airs-cli/src/airs/types.ts:1253

List latest topic revisions with client-side grouping in the SDK.

#### Parameters

##### opts?

[`PaginationOptions`](PaginationOptions.md)

#### Returns

`Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodDefault`\<`ZodArray`\<`ZodString`, `"many"`\>\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>[]\>

***

### listProfiles()

> **listProfiles**(`opts?`): `Promise`\<[`SecurityProfileListResult`](SecurityProfileListResult.md)\>

Defined in: prisma-airs-cli/src/airs/types.ts:1279

List security profiles.

#### Parameters

##### opts?

[`PaginationOptions`](PaginationOptions.md)

#### Returns

`Promise`\<[`SecurityProfileListResult`](SecurityProfileListResult.md)\>

***

### listTopics()

> **listTopics**(): `Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodDefault`\<`ZodArray`\<`ZodString`, `"many"`\>\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>[]\>

Defined in: prisma-airs-cli/src/airs/types.ts:1251

List all custom topics.

#### Returns

`Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodDefault`\<`ZodArray`\<`ZodString`, `"many"`\>\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>[]\>

***

### queryScanLogs()

> **queryScanLogs**(`opts`): `Promise`\<[`ScanLogQueryResult`](ScanLogQueryResult.md)\>

Defined in: prisma-airs-cli/src/airs/types.ts:1331

#### Parameters

##### opts

[`ScanLogQueryOptions`](ScanLogQueryOptions.md)

#### Returns

`Promise`\<[`ScanLogQueryResult`](ScanLogQueryResult.md)\>

***

### regenerateApiKey()

> **regenerateApiKey**(`apiKeyId`, `request`): `Promise`\<[`ApiKeyInfo`](ApiKeyInfo.md)\>

Defined in: prisma-airs-cli/src/airs/types.ts:1300

#### Parameters

##### apiKeyId

`string`

##### request

`Record`\<`string`, `unknown`\>

#### Returns

`Promise`\<[`ApiKeyInfo`](ApiKeyInfo.md)\>

***

### updateCustomerApp()

> **updateCustomerApp**(`appId`, `request`): `Promise`\<[`CustomerAppInfo`](CustomerAppInfo.md)\>

Defined in: prisma-airs-cli/src/airs/types.ts:1307

#### Parameters

##### appId

`string`

##### request

`Record`\<`string`, `unknown`\>

#### Returns

`Promise`\<[`CustomerAppInfo`](CustomerAppInfo.md)\>

***

### updateProfile()

> **updateProfile**(`profileId`, `request`): `Promise`\<[`SecurityProfileInfo`](SecurityProfileInfo.md)\>

Defined in: prisma-airs-cli/src/airs/types.ts:1287

Update a security profile.

#### Parameters

##### profileId

`string`

##### request

`CreateSecurityProfileRequest`

#### Returns

`Promise`\<[`SecurityProfileInfo`](SecurityProfileInfo.md)\>

***

### updateTopic()

> **updateTopic**(`topicId`, `request`): `Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodDefault`\<`ZodArray`\<`ZodString`, `"many"`\>\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>\>

Defined in: prisma-airs-cli/src/airs/types.ts:1245

Update an existing custom topic by ID.

#### Parameters

##### topicId

`string`

##### request

`objectOutputType`

#### Returns

`Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodDefault`\<`ZodArray`\<`ZodString`, `"many"`\>\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>\>
