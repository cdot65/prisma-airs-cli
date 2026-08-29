# Interface: ManagementService

Defined in: src/airs/types.ts:1238

Contract for AIRS topic CRUD, profile CRUD, and profile linking operations.

## Methods

### assignTopicsToProfile()

> **assignTopicsToProfile**(`profileName`, `topics`, `guardrailAction?`): `Promise`\<`void`\>

Defined in: src/airs/types.ts:1263

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

Defined in: src/airs/types.ts:1256

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

Defined in: src/airs/types.ts:1296

#### Parameters

##### request

`Record`\<`string`, `unknown`\>

#### Returns

`Promise`\<[`ApiKeyInfo`](ApiKeyInfo.md)\>

***

### createProfile()

> **createProfile**(`request`): `Promise`\<[`SecurityProfileInfo`](SecurityProfileInfo.md)\>

Defined in: src/airs/types.ts:1282

Create a security profile.

#### Parameters

##### request

`CreateSecurityProfileRequest`

#### Returns

`Promise`\<[`SecurityProfileInfo`](SecurityProfileInfo.md)\>

***

### createTopic()

> **createTopic**(`request`): `Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodArray`\<`ZodString`, `"many"`\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>\>

Defined in: src/airs/types.ts:1240

Create a new custom topic.

#### Parameters

##### request

`objectOutputType`

#### Returns

`Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodArray`\<`ZodString`, `"many"`\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>\>

***

### deleteApiKey()

> **deleteApiKey**(`apiKeyName`, `updatedBy`): `Promise`\<[`DeleteResponse`](DeleteResponse.md)\>

Defined in: src/airs/types.ts:1298

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

Defined in: src/airs/types.ts:1305

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

Defined in: src/airs/types.ts:1289

Delete a security profile.

#### Parameters

##### profileId

`string`

#### Returns

`Promise`\<[`DeleteResponse`](DeleteResponse.md)\>

***

### deleteTopic()

> **deleteTopic**(`topicId`): `Promise`\<`void`\>

Defined in: src/airs/types.ts:1244

Delete a custom topic by ID.

#### Parameters

##### topicId

`string`

#### Returns

`Promise`\<`void`\>

***

### forceDeleteProfile()

> **forceDeleteProfile**(`profileId`, `updatedBy`): `Promise`\<[`DeleteResponse`](DeleteResponse.md)\>

Defined in: src/airs/types.ts:1291

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

Defined in: src/airs/types.ts:1246

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

Defined in: src/airs/types.ts:1303

#### Parameters

##### appName

`string`

#### Returns

`Promise`\<[`CustomerAppInfo`](CustomerAppInfo.md)\>

***

### getCustomerAppConsumption()

> **getCustomerAppConsumption**(`appName`, `opts?`): `Promise`\<[`CustomerAppConsumption`](CustomerAppConsumption.md)\>

Defined in: src/airs/types.ts:1307

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

Defined in: src/airs/types.ts:1272

Get a single security profile by UUID.

#### Parameters

##### profileId

`string`

#### Returns

`Promise`\<[`SecurityProfileInfo`](SecurityProfileInfo.md)\>

***

### getProfileByName()

> **getProfileByName**(`profileName`): `Promise`\<[`SecurityProfileInfo`](SecurityProfileInfo.md)\>

Defined in: src/airs/types.ts:1274

Get a single security profile by name (returns highest revision).

#### Parameters

##### profileName

`string`

#### Returns

`Promise`\<[`SecurityProfileInfo`](SecurityProfileInfo.md)\>

***

### getProfileTopics()

> **getProfileTopics**(`profileName`): `Promise`\<[`ProfileTopic`](ProfileTopic.md)[]\>

Defined in: src/airs/types.ts:1269

List all topics configured in a profile with full details.

#### Parameters

##### profileName

`string`

#### Returns

`Promise`\<[`ProfileTopic`](ProfileTopic.md)[]\>

***

### getTopic()

> **getTopic**(`topicId`): `Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodArray`\<`ZodString`, `"many"`\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>\>

Defined in: src/airs/types.ts:1252

Get a single custom topic by ID.

#### Parameters

##### topicId

`string`

#### Returns

`Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodArray`\<`ZodString`, `"many"`\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>\>

***

### getTopicByName()

> **getTopicByName**(`topicName`): `Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodArray`\<`ZodString`, `"many"`\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>\>

Defined in: src/airs/types.ts:1254

Get a single custom topic by name.

#### Parameters

##### topicName

`string`

#### Returns

`Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodArray`\<`ZodString`, `"many"`\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>\>

***

### listAllApiKeys()

> **listAllApiKeys**(`opts?`): `Promise`\<[`ApiKeyInfo`](ApiKeyInfo.md)[]\>

Defined in: src/airs/types.ts:1295

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

Defined in: src/airs/types.ts:1302

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

Defined in: src/airs/types.ts:1278

Walk all security-profile pages.

#### Parameters

##### opts?

`Omit`\<[`PaginationOptions`](PaginationOptions.md), `"offset"`\> & `object`

#### Returns

`Promise`\<[`SecurityProfileInfo`](SecurityProfileInfo.md)[]\>

***

### listApiKeys()

> **listApiKeys**(`opts?`): `Promise`\<[`ApiKeyListResult`](ApiKeyListResult.md)\>

Defined in: src/airs/types.ts:1294

#### Parameters

##### opts?

[`PaginationOptions`](PaginationOptions.md)

#### Returns

`Promise`\<[`ApiKeyListResult`](ApiKeyListResult.md)\>

***

### listConsumptionApps()

> **listConsumptionApps**(`opts?`): `Promise`\<[`ConsumptionAppListEntry`](ConsumptionAppListEntry.md)[]\>

Defined in: src/airs/types.ts:1319

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

Defined in: src/airs/types.ts:1301

#### Parameters

##### opts?

[`PaginationOptions`](PaginationOptions.md)

#### Returns

`Promise`\<[`CustomerAppListResult`](CustomerAppListResult.md)\>

***

### listDeploymentProfiles()

> **listDeploymentProfiles**(`opts?`): `Promise`\<[`DeploymentProfileInfo`](DeploymentProfileInfo.md)[]\>

Defined in: src/airs/types.ts:1325

#### Parameters

##### opts?

###### unactivated?

`boolean`

#### Returns

`Promise`\<[`DeploymentProfileInfo`](DeploymentProfileInfo.md)[]\>

***

### listLatestTopics()

> **listLatestTopics**(`opts?`): `Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodArray`\<`ZodString`, `"many"`\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>[]\>

Defined in: src/airs/types.ts:1250

List latest topic revisions with client-side grouping in the SDK.

#### Parameters

##### opts?

[`PaginationOptions`](PaginationOptions.md)

#### Returns

`Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodArray`\<`ZodString`, `"many"`\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>[]\>

***

### listProfiles()

> **listProfiles**(`opts?`): `Promise`\<[`SecurityProfileListResult`](SecurityProfileListResult.md)\>

Defined in: src/airs/types.ts:1276

List security profiles.

#### Parameters

##### opts?

[`PaginationOptions`](PaginationOptions.md)

#### Returns

`Promise`\<[`SecurityProfileListResult`](SecurityProfileListResult.md)\>

***

### listTopics()

> **listTopics**(): `Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodArray`\<`ZodString`, `"many"`\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>[]\>

Defined in: src/airs/types.ts:1248

List all custom topics.

#### Returns

`Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodArray`\<`ZodString`, `"many"`\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>[]\>

***

### queryScanLogs()

> **queryScanLogs**(`opts`): `Promise`\<[`ScanLogQueryResult`](ScanLogQueryResult.md)\>

Defined in: src/airs/types.ts:1328

#### Parameters

##### opts

[`ScanLogQueryOptions`](ScanLogQueryOptions.md)

#### Returns

`Promise`\<[`ScanLogQueryResult`](ScanLogQueryResult.md)\>

***

### regenerateApiKey()

> **regenerateApiKey**(`apiKeyId`, `request`): `Promise`\<[`ApiKeyInfo`](ApiKeyInfo.md)\>

Defined in: src/airs/types.ts:1297

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

Defined in: src/airs/types.ts:1304

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

Defined in: src/airs/types.ts:1284

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

> **updateTopic**(`topicId`, `request`): `Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodArray`\<`ZodString`, `"many"`\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>\>

Defined in: src/airs/types.ts:1242

Update an existing custom topic by ID.

#### Parameters

##### topicId

`string`

##### request

`objectOutputType`

#### Returns

`Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodArray`\<`ZodString`, `"many"`\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>\>
