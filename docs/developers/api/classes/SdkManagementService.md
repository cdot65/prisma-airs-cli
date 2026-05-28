# Class: SdkManagementService

Defined in: [src/airs/management.ts:28](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L28)

Wraps the SDK's ManagementClient to implement our ManagementService interface.
OAuth2 token management, caching, and retry are handled by the SDK.

## Implements

- `ManagementService`

## Constructors

### Constructor

> **new SdkManagementService**(`opts?`): `SdkManagementService`

Defined in: [src/airs/management.ts:31](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L31)

#### Parameters

##### opts?

`ManagementClientOptions`

#### Returns

`SdkManagementService`

## Methods

### assignTopicsToProfile()

> **assignTopicsToProfile**(`profileName`, `topics`, `guardrailAction?`): `Promise`\<`void`\>

Defined in: [src/airs/management.ts:93](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L93)

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

Defined in: [src/airs/management.ts:75](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L75)

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

Defined in: [src/airs/management.ts:313](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L313)

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

Defined in: [src/airs/management.ts:265](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L265)

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

Defined in: [src/airs/management.ts:35](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L35)

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

Defined in: [src/airs/management.ts:323](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L323)

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

Defined in: [src/airs/management.ts:364](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L364)

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

Defined in: [src/airs/management.ts:278](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L278)

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

Defined in: [src/airs/management.ts:43](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L43)

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

Defined in: [src/airs/management.ts:283](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L283)

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

Defined in: [src/airs/management.ts:47](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L47)

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

Defined in: [src/airs/management.ts:351](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L351)

#### Parameters

##### appName

`string`

#### Returns

`Promise`\<`CustomerAppInfo`\>

#### Implementation of

`ManagementService.getCustomerApp`

***

### getProfile()

> **getProfile**(`profileId`): `Promise`\<`SecurityProfileInfo`\>

Defined in: [src/airs/management.ts:245](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L245)

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

Defined in: [src/airs/management.ts:250](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L250)

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

Defined in: [src/airs/management.ts:173](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L173)

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

Defined in: [src/airs/management.ts:57](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L57)

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

Defined in: [src/airs/management.ts:64](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L64)

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

Defined in: [src/airs/management.ts:303](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L303)

#### Parameters

##### opts?

`PaginationOptions`

#### Returns

`Promise`\<`ApiKeyListResult`\>

#### Implementation of

`ManagementService.listApiKeys`

***

### listCustomerApps()

> **listCustomerApps**(`opts?`): `Promise`\<`CustomerAppListResult`\>

Defined in: [src/airs/management.ts:341](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L341)

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

Defined in: [src/airs/management.ts:373](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L373)

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

Defined in: [src/airs/management.ts:255](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L255)

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

Defined in: [src/airs/management.ts:52](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L52)

List all custom topics.

#### Returns

`Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodArray`\<`ZodString`, `"many"`\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>[]\>

#### Implementation of

`ManagementService.listTopics`

***

### queryScanLogs()

> **queryScanLogs**(`opts`): `Promise`\<`ScanLogQueryResult`\>

Defined in: [src/airs/management.ts:384](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L384)

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

Defined in: [src/airs/management.ts:318](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L318)

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

Defined in: [src/airs/management.ts:356](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L356)

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

Defined in: [src/airs/management.ts:270](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L270)

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

Defined in: [src/airs/management.ts:39](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/management.ts#L39)

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
