# Class: SdkManagementService

Defined in: [src/airs/management.ts:31](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L31)

Wraps the SDK's ManagementClient to implement our ManagementService interface.
OAuth2 token management, caching, and retry are handled by the SDK.

## Implements

- `ManagementService`

## Constructors

### Constructor

> **new SdkManagementService**(`opts?`): `SdkManagementService`

Defined in: [src/airs/management.ts:34](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L34)

#### Parameters

##### opts?

`ManagementClientOptions`

#### Returns

`SdkManagementService`

## Methods

### assignTopicsToProfile()

> **assignTopicsToProfile**(`profileName`, `topics`, `guardrailAction?`): `Promise`\<`void`\>

Defined in: [src/airs/management.ts:96](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L96)

Sets one or more custom topics on a profile's topic-guardrails config.
Replaces any existing topics — previous runs' stale topics are cleared.
Groups topics by action; skips empty action groups (AIRS rejects them).

CRITICAL: Each topic entry MUST include the current `revision` number.
AIRS pins topic content to the revision specified in the profile — omitting
it defaults to revision 0 (original content), not the latest.

#### Parameters

##### profileName

`string`

##### topics

`object`[]

##### guardrailAction?

`"allow"` \| `"block"`

#### Returns

`Promise`\<`void`\>

#### Implementation of

`ManagementService.assignTopicsToProfile`

***

### assignTopicToProfile()

> **assignTopicToProfile**(`profileName`, `topicId`, `topicName`, `action`): `Promise`\<`void`\>

Defined in: [src/airs/management.ts:78](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L78)

Sets a single custom topic on a profile's topic-guardrails config.
Delegates to [assignTopicsToProfile](#assigntopicstoprofile) for backward compatibility.

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

#### Implementation of

`ManagementService.assignTopicToProfile`

***

### createApiKey()

> **createApiKey**(`request`): `Promise`\<`ApiKeyInfo`\>

Defined in: [src/airs/management.ts:316](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L316)

#### Parameters

##### request

`Record`\<`string`, `unknown`\>

#### Returns

`Promise`\<`ApiKeyInfo`\>

#### Implementation of

`ManagementService.createApiKey`

***

### createProfile()

> **createProfile**(`request`): `Promise`\<`SecurityProfileInfo`\>

Defined in: [src/airs/management.ts:268](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L268)

Create a security profile.

#### Parameters

##### request

`CreateSecurityProfileRequest`

#### Returns

`Promise`\<`SecurityProfileInfo`\>

#### Implementation of

`ManagementService.createProfile`

***

### createTopic()

> **createTopic**(`request`): `Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodArray`\<`ZodString`, `"many"`\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>\>

Defined in: [src/airs/management.ts:38](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L38)

Create a new custom topic.

#### Parameters

##### request

`objectOutputType`

#### Returns

`Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodArray`\<`ZodString`, `"many"`\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>\>

#### Implementation of

`ManagementService.createTopic`

***

### deleteApiKey()

> **deleteApiKey**(`apiKeyName`, `updatedBy`): `Promise`\<`DeleteResponse`\>

Defined in: [src/airs/management.ts:326](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L326)

#### Parameters

##### apiKeyName

`string`

##### updatedBy

`string`

#### Returns

`Promise`\<`DeleteResponse`\>

#### Implementation of

`ManagementService.deleteApiKey`

***

### deleteCustomerApp()

> **deleteCustomerApp**(`appName`, `updatedBy`): `Promise`\<`CustomerAppInfo`\>

Defined in: [src/airs/management.ts:367](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L367)

#### Parameters

##### appName

`string`

##### updatedBy

`string`

#### Returns

`Promise`\<`CustomerAppInfo`\>

#### Implementation of

`ManagementService.deleteCustomerApp`

***

### deleteProfile()

> **deleteProfile**(`profileId`): `Promise`\<`DeleteResponse`\>

Defined in: [src/airs/management.ts:281](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L281)

Delete a security profile.

#### Parameters

##### profileId

`string`

#### Returns

`Promise`\<`DeleteResponse`\>

#### Implementation of

`ManagementService.deleteProfile`

***

### deleteTopic()

> **deleteTopic**(`topicId`): `Promise`\<`void`\>

Defined in: [src/airs/management.ts:46](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L46)

Delete a custom topic by ID.

#### Parameters

##### topicId

`string`

#### Returns

`Promise`\<`void`\>

#### Implementation of

`ManagementService.deleteTopic`

***

### forceDeleteProfile()

> **forceDeleteProfile**(`profileId`, `updatedBy`): `Promise`\<`DeleteResponse`\>

Defined in: [src/airs/management.ts:286](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L286)

Force-delete a security profile (removes from referencing policies).

#### Parameters

##### profileId

`string`

##### updatedBy

`string`

#### Returns

`Promise`\<`DeleteResponse`\>

#### Implementation of

`ManagementService.forceDeleteProfile`

***

### forceDeleteTopic()

> **forceDeleteTopic**(`topicId`, `updatedBy?`): `Promise`\<`DeleteResponse`\>

Defined in: [src/airs/management.ts:50](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L50)

Force-delete a custom topic (removes from all referencing profiles).

#### Parameters

##### topicId

`string`

##### updatedBy?

`string`

#### Returns

`Promise`\<`DeleteResponse`\>

#### Implementation of

`ManagementService.forceDeleteTopic`

***

### getCustomerApp()

> **getCustomerApp**(`appName`): `Promise`\<`CustomerAppInfo`\>

Defined in: [src/airs/management.ts:354](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L354)

#### Parameters

##### appName

`string`

#### Returns

`Promise`\<`CustomerAppInfo`\>

#### Implementation of

`ManagementService.getCustomerApp`

***

### getCustomerAppConsumption()

> **getCustomerAppConsumption**(`appName`, `opts?`): `Promise`\<`CustomerAppConsumption`\>

Defined in: [src/airs/management.ts:397](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L397)

Get per-app token consumption + violation breakdown from the SCM dashboard endpoints.

#### Parameters

##### appName

`string`

##### opts?

`ConsumptionQueryOptions`

#### Returns

`Promise`\<`CustomerAppConsumption`\>

#### Implementation of

`ManagementService.getCustomerAppConsumption`

***

### getProfile()

> **getProfile**(`profileId`): `Promise`\<`SecurityProfileInfo`\>

Defined in: [src/airs/management.ts:248](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L248)

Get a single security profile by UUID.

#### Parameters

##### profileId

`string`

#### Returns

`Promise`\<`SecurityProfileInfo`\>

#### Implementation of

`ManagementService.getProfile`

***

### getProfileByName()

> **getProfileByName**(`profileName`): `Promise`\<`SecurityProfileInfo`\>

Defined in: [src/airs/management.ts:253](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L253)

Get a single security profile by name (returns highest revision).

#### Parameters

##### profileName

`string`

#### Returns

`Promise`\<`SecurityProfileInfo`\>

#### Implementation of

`ManagementService.getProfileByName`

***

### getProfileTopics()

> **getProfileTopics**(`profileName`): `Promise`\<[`ProfileTopic`](../interfaces/ProfileTopic.md)[]\>

Defined in: [src/airs/management.ts:176](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L176)

List all topics configured in a profile with full details.

#### Parameters

##### profileName

`string`

#### Returns

`Promise`\<[`ProfileTopic`](../interfaces/ProfileTopic.md)[]\>

#### Implementation of

`ManagementService.getProfileTopics`

***

### getTopic()

> **getTopic**(`topicId`): `Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodArray`\<`ZodString`, `"many"`\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>\>

Defined in: [src/airs/management.ts:60](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L60)

Get a single custom topic by ID.

#### Parameters

##### topicId

`string`

#### Returns

`Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodArray`\<`ZodString`, `"many"`\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>\>

#### Implementation of

`ManagementService.getTopic`

***

### getTopicByName()

> **getTopicByName**(`topicName`): `Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodArray`\<`ZodString`, `"many"`\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>\>

Defined in: [src/airs/management.ts:67](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L67)

Get a single custom topic by name.

#### Parameters

##### topicName

`string`

#### Returns

`Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodArray`\<`ZodString`, `"many"`\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>\>

#### Implementation of

`ManagementService.getTopicByName`

***

### listApiKeys()

> **listApiKeys**(`opts?`): `Promise`\<`ApiKeyListResult`\>

Defined in: [src/airs/management.ts:306](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L306)

#### Parameters

##### opts?

`PaginationOptions`

#### Returns

`Promise`\<`ApiKeyListResult`\>

#### Implementation of

`ManagementService.listApiKeys`

***

### listConsumptionApps()

> **listConsumptionApps**(`opts?`): `Promise`\<`ConsumptionAppListEntry`[]\>

Defined in: [src/airs/management.ts:372](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L372)

List dashboard application buckets - the canonical apps source for consumption reporting.

Drawn from `dashboard.applicationsOverview`. One entry per dashboard bucket, which is one
per distinct scan-payload `metadata.app_name` per registered customer-app. Distinct from
ManagementService.listCustomerApps, which enumerates registered customer-apps
(different granularity).

#### Parameters

##### opts?

###### limit?

`number`

###### offset?

`number`

#### Returns

`Promise`\<`ConsumptionAppListEntry`[]\>

#### Implementation of

`ManagementService.listConsumptionApps`

***

### listCustomerApps()

> **listCustomerApps**(`opts?`): `Promise`\<`CustomerAppListResult`\>

Defined in: [src/airs/management.ts:344](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L344)

#### Parameters

##### opts?

`PaginationOptions`

#### Returns

`Promise`\<`CustomerAppListResult`\>

#### Implementation of

`ManagementService.listCustomerApps`

***

### listDeploymentProfiles()

> **listDeploymentProfiles**(`opts?`): `Promise`\<`DeploymentProfileInfo`[]\>

Defined in: [src/airs/management.ts:471](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L471)

#### Parameters

##### opts?

###### unactivated?

`boolean`

#### Returns

`Promise`\<`DeploymentProfileInfo`[]\>

#### Implementation of

`ManagementService.listDeploymentProfiles`

***

### listProfiles()

> **listProfiles**(`opts?`): `Promise`\<`SecurityProfileListResult`\>

Defined in: [src/airs/management.ts:258](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L258)

List security profiles.

#### Parameters

##### opts?

`PaginationOptions`

#### Returns

`Promise`\<`SecurityProfileListResult`\>

#### Implementation of

`ManagementService.listProfiles`

***

### listTopics()

> **listTopics**(): `Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodArray`\<`ZodString`, `"many"`\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>[]\>

Defined in: [src/airs/management.ts:55](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L55)

List all custom topics.

#### Returns

`Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodArray`\<`ZodString`, `"many"`\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>[]\>

#### Implementation of

`ManagementService.listTopics`

***

### queryScanLogs()

> **queryScanLogs**(`opts`): `Promise`\<`ScanLogQueryResult`\>

Defined in: [src/airs/management.ts:482](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L482)

#### Parameters

##### opts

`ScanLogQueryOptions`

#### Returns

`Promise`\<`ScanLogQueryResult`\>

#### Implementation of

`ManagementService.queryScanLogs`

***

### regenerateApiKey()

> **regenerateApiKey**(`apiKeyId`, `request`): `Promise`\<`ApiKeyInfo`\>

Defined in: [src/airs/management.ts:321](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L321)

#### Parameters

##### apiKeyId

`string`

##### request

`Record`\<`string`, `unknown`\>

#### Returns

`Promise`\<`ApiKeyInfo`\>

#### Implementation of

`ManagementService.regenerateApiKey`

***

### updateCustomerApp()

> **updateCustomerApp**(`appId`, `request`): `Promise`\<`CustomerAppInfo`\>

Defined in: [src/airs/management.ts:359](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L359)

#### Parameters

##### appId

`string`

##### request

`Record`\<`string`, `unknown`\>

#### Returns

`Promise`\<`CustomerAppInfo`\>

#### Implementation of

`ManagementService.updateCustomerApp`

***

### updateProfile()

> **updateProfile**(`profileId`, `request`): `Promise`\<`SecurityProfileInfo`\>

Defined in: [src/airs/management.ts:273](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L273)

Update a security profile.

#### Parameters

##### profileId

`string`

##### request

`CreateSecurityProfileRequest`

#### Returns

`Promise`\<`SecurityProfileInfo`\>

#### Implementation of

`ManagementService.updateProfile`

***

### updateTopic()

> **updateTopic**(`topicId`, `request`): `Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodArray`\<`ZodString`, `"many"`\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>\>

Defined in: [src/airs/management.ts:42](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L42)

Update an existing custom topic by ID.

#### Parameters

##### topicId

`string`

##### request

`objectOutputType`

#### Returns

`Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodArray`\<`ZodString`, `"many"`\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>\>

#### Implementation of

`ManagementService.updateTopic`
