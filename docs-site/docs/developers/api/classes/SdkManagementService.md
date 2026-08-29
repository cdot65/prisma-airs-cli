# Class: SdkManagementService

Defined in: src/airs/management.ts:31

Wraps the SDK's ManagementClient to implement our ManagementService interface.
OAuth2 token management, caching, and retry are handled by the SDK.

## Implements

- [`ManagementService`](../interfaces/ManagementService.md)

## Constructors

### Constructor

> **new SdkManagementService**(`opts?`): `SdkManagementService`

Defined in: src/airs/management.ts:34

#### Parameters

##### opts?

`ManagementClientOptions`

#### Returns

`SdkManagementService`

## Methods

### assignTopicsToProfile()

> **assignTopicsToProfile**(`profileName`, `topics`, `guardrailAction?`): `Promise`\<`void`\>

Defined in: src/airs/management.ts:104

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

[`ManagementService`](../interfaces/ManagementService.md).[`assignTopicsToProfile`](../interfaces/ManagementService.md#assigntopicstoprofile)

***

### assignTopicToProfile()

> **assignTopicToProfile**(`profileName`, `topicId`, `topicName`, `action`): `Promise`\<`void`\>

Defined in: src/airs/management.ts:86

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

[`ManagementService`](../interfaces/ManagementService.md).[`assignTopicToProfile`](../interfaces/ManagementService.md#assigntopictoprofile)

***

### createApiKey()

> **createApiKey**(`request`): `Promise`\<[`ApiKeyInfo`](../interfaces/ApiKeyInfo.md)\>

Defined in: src/airs/management.ts:339

#### Parameters

##### request

`Record`\<`string`, `unknown`\>

#### Returns

`Promise`\<[`ApiKeyInfo`](../interfaces/ApiKeyInfo.md)\>

#### Implementation of

[`ManagementService`](../interfaces/ManagementService.md).[`createApiKey`](../interfaces/ManagementService.md#createapikey)

***

### createProfile()

> **createProfile**(`request`): `Promise`\<[`SecurityProfileInfo`](../interfaces/SecurityProfileInfo.md)\>

Defined in: src/airs/management.ts:285

Create a security profile.

#### Parameters

##### request

`CreateSecurityProfileRequest`

#### Returns

`Promise`\<[`SecurityProfileInfo`](../interfaces/SecurityProfileInfo.md)\>

#### Implementation of

[`ManagementService`](../interfaces/ManagementService.md).[`createProfile`](../interfaces/ManagementService.md#createprofile)

***

### createTopic()

> **createTopic**(`request`): `Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodArray`\<`ZodString`, `"many"`\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>\>

Defined in: src/airs/management.ts:38

Create a new custom topic.

#### Parameters

##### request

`objectOutputType`

#### Returns

`Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodArray`\<`ZodString`, `"many"`\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>\>

#### Implementation of

[`ManagementService`](../interfaces/ManagementService.md).[`createTopic`](../interfaces/ManagementService.md#createtopic)

***

### deleteApiKey()

> **deleteApiKey**(`apiKeyName`, `updatedBy`): `Promise`\<[`DeleteResponse`](../interfaces/DeleteResponse.md)\>

Defined in: src/airs/management.ts:349

#### Parameters

##### apiKeyName

`string`

##### updatedBy

`string`

#### Returns

`Promise`\<[`DeleteResponse`](../interfaces/DeleteResponse.md)\>

#### Implementation of

[`ManagementService`](../interfaces/ManagementService.md).[`deleteApiKey`](../interfaces/ManagementService.md#deleteapikey)

***

### deleteCustomerApp()

> **deleteCustomerApp**(`appName`, `updatedBy`): `Promise`\<[`CustomerAppInfo`](../interfaces/CustomerAppInfo.md)\>

Defined in: src/airs/management.ts:396

#### Parameters

##### appName

`string`

##### updatedBy

`string`

#### Returns

`Promise`\<[`CustomerAppInfo`](../interfaces/CustomerAppInfo.md)\>

#### Implementation of

[`ManagementService`](../interfaces/ManagementService.md).[`deleteCustomerApp`](../interfaces/ManagementService.md#deletecustomerapp)

***

### deleteProfile()

> **deleteProfile**(`profileId`): `Promise`\<[`DeleteResponse`](../interfaces/DeleteResponse.md)\>

Defined in: src/airs/management.ts:298

Delete a security profile.

#### Parameters

##### profileId

`string`

#### Returns

`Promise`\<[`DeleteResponse`](../interfaces/DeleteResponse.md)\>

#### Implementation of

[`ManagementService`](../interfaces/ManagementService.md).[`deleteProfile`](../interfaces/ManagementService.md#deleteprofile)

***

### deleteTopic()

> **deleteTopic**(`topicId`): `Promise`\<`void`\>

Defined in: src/airs/management.ts:46

Delete a custom topic by ID.

#### Parameters

##### topicId

`string`

#### Returns

`Promise`\<`void`\>

#### Implementation of

[`ManagementService`](../interfaces/ManagementService.md).[`deleteTopic`](../interfaces/ManagementService.md#deletetopic)

***

### forceDeleteProfile()

> **forceDeleteProfile**(`profileId`, `updatedBy`): `Promise`\<[`DeleteResponse`](../interfaces/DeleteResponse.md)\>

Defined in: src/airs/management.ts:303

Force-delete a security profile (removes from referencing policies).

#### Parameters

##### profileId

`string`

##### updatedBy

`string`

#### Returns

`Promise`\<[`DeleteResponse`](../interfaces/DeleteResponse.md)\>

#### Implementation of

[`ManagementService`](../interfaces/ManagementService.md).[`forceDeleteProfile`](../interfaces/ManagementService.md#forcedeleteprofile)

***

### forceDeleteTopic()

> **forceDeleteTopic**(`topicId`, `updatedBy?`): `Promise`\<[`DeleteResponse`](../interfaces/DeleteResponse.md)\>

Defined in: src/airs/management.ts:50

Force-delete a custom topic (removes from all referencing profiles).

#### Parameters

##### topicId

`string`

##### updatedBy?

`string`

#### Returns

`Promise`\<[`DeleteResponse`](../interfaces/DeleteResponse.md)\>

#### Implementation of

[`ManagementService`](../interfaces/ManagementService.md).[`forceDeleteTopic`](../interfaces/ManagementService.md#forcedeletetopic)

***

### getCustomerApp()

> **getCustomerApp**(`appName`): `Promise`\<[`CustomerAppInfo`](../interfaces/CustomerAppInfo.md)\>

Defined in: src/airs/management.ts:383

#### Parameters

##### appName

`string`

#### Returns

`Promise`\<[`CustomerAppInfo`](../interfaces/CustomerAppInfo.md)\>

#### Implementation of

[`ManagementService`](../interfaces/ManagementService.md).[`getCustomerApp`](../interfaces/ManagementService.md#getcustomerapp)

***

### getCustomerAppConsumption()

> **getCustomerAppConsumption**(`appName`, `opts?`): `Promise`\<[`CustomerAppConsumption`](../interfaces/CustomerAppConsumption.md)\>

Defined in: src/airs/management.ts:435

Get per-app token consumption + violation breakdown from the SCM dashboard endpoints.

#### Parameters

##### appName

`string`

##### opts?

[`ConsumptionQueryOptions`](../interfaces/ConsumptionQueryOptions.md)

#### Returns

`Promise`\<[`CustomerAppConsumption`](../interfaces/CustomerAppConsumption.md)\>

#### Implementation of

[`ManagementService`](../interfaces/ManagementService.md).[`getCustomerAppConsumption`](../interfaces/ManagementService.md#getcustomerappconsumption)

***

### getProfile()

> **getProfile**(`profileId`): `Promise`\<[`SecurityProfileInfo`](../interfaces/SecurityProfileInfo.md)\>

Defined in: src/airs/management.ts:256

Get a single security profile by UUID.

#### Parameters

##### profileId

`string`

#### Returns

`Promise`\<[`SecurityProfileInfo`](../interfaces/SecurityProfileInfo.md)\>

#### Implementation of

[`ManagementService`](../interfaces/ManagementService.md).[`getProfile`](../interfaces/ManagementService.md#getprofile)

***

### getProfileByName()

> **getProfileByName**(`profileName`): `Promise`\<[`SecurityProfileInfo`](../interfaces/SecurityProfileInfo.md)\>

Defined in: src/airs/management.ts:261

Get a single security profile by name (returns highest revision).

#### Parameters

##### profileName

`string`

#### Returns

`Promise`\<[`SecurityProfileInfo`](../interfaces/SecurityProfileInfo.md)\>

#### Implementation of

[`ManagementService`](../interfaces/ManagementService.md).[`getProfileByName`](../interfaces/ManagementService.md#getprofilebyname)

***

### getProfileTopics()

> **getProfileTopics**(`profileName`): `Promise`\<[`ProfileTopic`](../interfaces/ProfileTopic.md)[]\>

Defined in: src/airs/management.ts:184

List all topics configured in a profile with full details.

#### Parameters

##### profileName

`string`

#### Returns

`Promise`\<[`ProfileTopic`](../interfaces/ProfileTopic.md)[]\>

#### Implementation of

[`ManagementService`](../interfaces/ManagementService.md).[`getProfileTopics`](../interfaces/ManagementService.md#getprofiletopics)

***

### getTopic()

> **getTopic**(`topicId`): `Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodArray`\<`ZodString`, `"many"`\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>\>

Defined in: src/airs/management.ts:68

Get a single custom topic by ID.

#### Parameters

##### topicId

`string`

#### Returns

`Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodArray`\<`ZodString`, `"many"`\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>\>

#### Implementation of

[`ManagementService`](../interfaces/ManagementService.md).[`getTopic`](../interfaces/ManagementService.md#gettopic)

***

### getTopicByName()

> **getTopicByName**(`topicName`): `Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodArray`\<`ZodString`, `"many"`\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>\>

Defined in: src/airs/management.ts:75

Get a single custom topic by name.

#### Parameters

##### topicName

`string`

#### Returns

`Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodArray`\<`ZodString`, `"many"`\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>\>

#### Implementation of

[`ManagementService`](../interfaces/ManagementService.md).[`getTopicByName`](../interfaces/ManagementService.md#gettopicbyname)

***

### listAllApiKeys()

> **listAllApiKeys**(`opts?`): `Promise`\<[`ApiKeyInfo`](../interfaces/ApiKeyInfo.md)[]\>

Defined in: src/airs/management.ts:333

#### Parameters

##### opts?

###### limit?

`number`

###### max?

`number`

#### Returns

`Promise`\<[`ApiKeyInfo`](../interfaces/ApiKeyInfo.md)[]\>

#### Implementation of

[`ManagementService`](../interfaces/ManagementService.md).[`listAllApiKeys`](../interfaces/ManagementService.md#listallapikeys)

***

### listAllCustomerApps()

> **listAllCustomerApps**(`opts?`): `Promise`\<[`CustomerAppInfo`](../interfaces/CustomerAppInfo.md)[]\>

Defined in: src/airs/management.ts:377

#### Parameters

##### opts?

###### limit?

`number`

###### max?

`number`

#### Returns

`Promise`\<[`CustomerAppInfo`](../interfaces/CustomerAppInfo.md)[]\>

#### Implementation of

[`ManagementService`](../interfaces/ManagementService.md).[`listAllCustomerApps`](../interfaces/ManagementService.md#listallcustomerapps)

***

### listAllProfiles()

> **listAllProfiles**(`opts?`): `Promise`\<[`SecurityProfileInfo`](../interfaces/SecurityProfileInfo.md)[]\>

Defined in: src/airs/management.ts:276

Walk all security-profile pages.

#### Parameters

##### opts?

`Omit`\<[`PaginationOptions`](../interfaces/PaginationOptions.md), `"offset"`\> & `object`

#### Returns

`Promise`\<[`SecurityProfileInfo`](../interfaces/SecurityProfileInfo.md)[]\>

#### Implementation of

[`ManagementService`](../interfaces/ManagementService.md).[`listAllProfiles`](../interfaces/ManagementService.md#listallprofiles)

***

### listApiKeys()

> **listApiKeys**(`opts?`): `Promise`\<[`ApiKeyListResult`](../interfaces/ApiKeyListResult.md)\>

Defined in: src/airs/management.ts:323

#### Parameters

##### opts?

[`PaginationOptions`](../interfaces/PaginationOptions.md)

#### Returns

`Promise`\<[`ApiKeyListResult`](../interfaces/ApiKeyListResult.md)\>

#### Implementation of

[`ManagementService`](../interfaces/ManagementService.md).[`listApiKeys`](../interfaces/ManagementService.md#listapikeys)

***

### listConsumptionApps()

> **listConsumptionApps**(`opts?`): `Promise`\<[`ConsumptionAppListEntry`](../interfaces/ConsumptionAppListEntry.md)[]\>

Defined in: src/airs/management.ts:401

List dashboard application buckets - the canonical apps source for consumption reporting.

Drawn from `dashboard.applicationsOverview`. One entry per dashboard bucket, which is one
per distinct scan-payload `metadata.app_name` per registered customer-app. Distinct from
[ManagementService.listCustomerApps](../interfaces/ManagementService.md#listcustomerapps), which enumerates registered customer-apps
(different granularity).

#### Parameters

##### opts?

###### limit?

`number`

###### offset?

`number`

#### Returns

`Promise`\<[`ConsumptionAppListEntry`](../interfaces/ConsumptionAppListEntry.md)[]\>

#### Implementation of

[`ManagementService`](../interfaces/ManagementService.md).[`listConsumptionApps`](../interfaces/ManagementService.md#listconsumptionapps)

***

### listCustomerApps()

> **listCustomerApps**(`opts?`): `Promise`\<[`CustomerAppListResult`](../interfaces/CustomerAppListResult.md)\>

Defined in: src/airs/management.ts:367

#### Parameters

##### opts?

[`PaginationOptions`](../interfaces/PaginationOptions.md)

#### Returns

`Promise`\<[`CustomerAppListResult`](../interfaces/CustomerAppListResult.md)\>

#### Implementation of

[`ManagementService`](../interfaces/ManagementService.md).[`listCustomerApps`](../interfaces/ManagementService.md#listcustomerapps)

***

### listDeploymentProfiles()

> **listDeploymentProfiles**(`opts?`): `Promise`\<[`DeploymentProfileInfo`](../interfaces/DeploymentProfileInfo.md)[]\>

Defined in: src/airs/management.ts:509

#### Parameters

##### opts?

###### unactivated?

`boolean`

#### Returns

`Promise`\<[`DeploymentProfileInfo`](../interfaces/DeploymentProfileInfo.md)[]\>

#### Implementation of

[`ManagementService`](../interfaces/ManagementService.md).[`listDeploymentProfiles`](../interfaces/ManagementService.md#listdeploymentprofiles)

***

### listLatestTopics()

> **listLatestTopics**(`opts?`): `Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodArray`\<`ZodString`, `"many"`\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>[]\>

Defined in: src/airs/management.ts:59

List latest topic revisions with client-side grouping in the SDK.

#### Parameters

##### opts?

[`PaginationOptions`](../interfaces/PaginationOptions.md)

#### Returns

`Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodArray`\<`ZodString`, `"many"`\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>[]\>

#### Implementation of

[`ManagementService`](../interfaces/ManagementService.md).[`listLatestTopics`](../interfaces/ManagementService.md#listlatesttopics)

***

### listProfiles()

> **listProfiles**(`opts?`): `Promise`\<[`SecurityProfileListResult`](../interfaces/SecurityProfileListResult.md)\>

Defined in: src/airs/management.ts:266

List security profiles.

#### Parameters

##### opts?

[`PaginationOptions`](../interfaces/PaginationOptions.md)

#### Returns

`Promise`\<[`SecurityProfileListResult`](../interfaces/SecurityProfileListResult.md)\>

#### Implementation of

[`ManagementService`](../interfaces/ManagementService.md).[`listProfiles`](../interfaces/ManagementService.md#listprofiles)

***

### listTopics()

> **listTopics**(): `Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodArray`\<`ZodString`, `"many"`\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>[]\>

Defined in: src/airs/management.ts:55

List all custom topics.

#### Returns

`Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodArray`\<`ZodString`, `"many"`\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>[]\>

#### Implementation of

[`ManagementService`](../interfaces/ManagementService.md).[`listTopics`](../interfaces/ManagementService.md#listtopics)

***

### queryScanLogs()

> **queryScanLogs**(`opts`): `Promise`\<[`ScanLogQueryResult`](../interfaces/ScanLogQueryResult.md)\>

Defined in: src/airs/management.ts:520

#### Parameters

##### opts

[`ScanLogQueryOptions`](../interfaces/ScanLogQueryOptions.md)

#### Returns

`Promise`\<[`ScanLogQueryResult`](../interfaces/ScanLogQueryResult.md)\>

#### Implementation of

[`ManagementService`](../interfaces/ManagementService.md).[`queryScanLogs`](../interfaces/ManagementService.md#queryscanlogs)

***

### regenerateApiKey()

> **regenerateApiKey**(`apiKeyId`, `request`): `Promise`\<[`ApiKeyInfo`](../interfaces/ApiKeyInfo.md)\>

Defined in: src/airs/management.ts:344

#### Parameters

##### apiKeyId

`string`

##### request

`Record`\<`string`, `unknown`\>

#### Returns

`Promise`\<[`ApiKeyInfo`](../interfaces/ApiKeyInfo.md)\>

#### Implementation of

[`ManagementService`](../interfaces/ManagementService.md).[`regenerateApiKey`](../interfaces/ManagementService.md#regenerateapikey)

***

### updateCustomerApp()

> **updateCustomerApp**(`appId`, `request`): `Promise`\<[`CustomerAppInfo`](../interfaces/CustomerAppInfo.md)\>

Defined in: src/airs/management.ts:388

#### Parameters

##### appId

`string`

##### request

`Record`\<`string`, `unknown`\>

#### Returns

`Promise`\<[`CustomerAppInfo`](../interfaces/CustomerAppInfo.md)\>

#### Implementation of

[`ManagementService`](../interfaces/ManagementService.md).[`updateCustomerApp`](../interfaces/ManagementService.md#updatecustomerapp)

***

### updateProfile()

> **updateProfile**(`profileId`, `request`): `Promise`\<[`SecurityProfileInfo`](../interfaces/SecurityProfileInfo.md)\>

Defined in: src/airs/management.ts:290

Update a security profile.

#### Parameters

##### profileId

`string`

##### request

`CreateSecurityProfileRequest`

#### Returns

`Promise`\<[`SecurityProfileInfo`](../interfaces/SecurityProfileInfo.md)\>

#### Implementation of

[`ManagementService`](../interfaces/ManagementService.md).[`updateProfile`](../interfaces/ManagementService.md#updateprofile)

***

### updateTopic()

> **updateTopic**(`topicId`, `request`): `Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodArray`\<`ZodString`, `"many"`\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>\>

Defined in: src/airs/management.ts:42

Update an existing custom topic by ID.

#### Parameters

##### topicId

`string`

##### request

`objectOutputType`

#### Returns

`Promise`\<`objectOutputType`\<\{ `active`: `ZodOptional`\<`ZodBoolean`\>; `created_by`: `ZodOptional`\<`ZodString`\>; `created_ts`: `ZodOptional`\<`ZodString`\>; `description`: `ZodString`; `examples`: `ZodArray`\<`ZodString`, `"many"`\>; `last_modified_ts`: `ZodOptional`\<`ZodString`\>; `revision`: `ZodNumber`; `topic_id`: `ZodOptional`\<`ZodString`\>; `topic_name`: `ZodString`; `updated_by`: `ZodOptional`\<`ZodString`\>; \}, `ZodTypeAny`, `"passthrough"`\>\>

#### Implementation of

[`ManagementService`](../interfaces/ManagementService.md).[`updateTopic`](../interfaces/ManagementService.md#updatetopic)
