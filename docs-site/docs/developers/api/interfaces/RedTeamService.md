# Interface: RedTeamService

Defined in: src/airs/types.ts:548

Contract for AI Red Team scan operations.

## Methods

### abortScan()

> **abortScan**(`jobId`): `Promise`\<`void`\>

Defined in: src/airs/types.ts:652

Abort a running scan.

#### Parameters

##### jobId

`string`

#### Returns

`Promise`\<`void`\>

***

### acceptEula()

> **acceptEula**(`eulaContent`): `Promise`\<[`EulaStatus`](EulaStatus.md)\>

Defined in: src/airs/types.ts:554

Accept the EULA.

#### Parameters

##### eulaContent

`string`

#### Returns

`Promise`\<[`EulaStatus`](EulaStatus.md)\>

***

### createAdapter()

> **createAdapter**(`request`, `validate?`): `Promise`\<[`RedTeamAdapterDetail`](RedTeamAdapterDetail.md)\>

Defined in: src/airs/types.ts:712

#### Parameters

##### request

[`RedTeamAdapterCreateRequest`](RedTeamAdapterCreateRequest.md)

##### validate?

`boolean`

#### Returns

`Promise`\<[`RedTeamAdapterDetail`](RedTeamAdapterDetail.md)\>

***

### createChannel()

> **createChannel**(`request`): `Promise`\<[`RedTeamChannel`](RedTeamChannel.md)\>

Defined in: src/airs/types.ts:690

Create a network broker channel.

#### Parameters

##### request

[`RedTeamChannelCreateRequest`](RedTeamChannelCreateRequest.md)

#### Returns

`Promise`\<[`RedTeamChannel`](RedTeamChannel.md)\>

***

### createDevices()

> **createDevices**(`tenantId`, `request`): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: src/airs/types.ts:565

Create devices for an instance.

#### Parameters

##### tenantId

`string`

##### request

`Record`\<`string`, `unknown`\>

#### Returns

`Promise`\<`Record`\<`string`, `unknown`\>\>

***

### createInstance()

> **createInstance**(`request`): `Promise`\<[`InstanceResponse`](InstanceResponse.md)\>

Defined in: src/airs/types.ts:557

Create an instance.

#### Parameters

##### request

[`InstanceRequest`](InstanceRequest.md)

#### Returns

`Promise`\<[`InstanceResponse`](InstanceResponse.md)\>

***

### createScan()

> **createScan**(`request`): `Promise`\<[`RedTeamJob`](RedTeamJob.md)\>

Defined in: src/airs/types.ts:621

Create a red team scan job.

#### Parameters

##### request

###### attackGoals?

`string`[]

###### categories?

`Record`\<`string`, `unknown`\>

###### customPromptSets?

`string`[]

###### jobType

`string`

###### name

`string`

###### streamBreadth?

`number`

###### streamDepth?

`number`

###### targetUuid

`string`

#### Returns

`Promise`\<[`RedTeamJob`](RedTeamJob.md)\>

***

### createTarget()

> **createTarget**(`request`, `opts?`): `Promise`\<[`RedTeamTargetDetail`](RedTeamTargetDetail.md)\>

Defined in: src/airs/types.ts:593

Create a red team target.

#### Parameters

##### request

[`RedTeamTargetCreateRequest`](RedTeamTargetCreateRequest.md)

##### opts?

[`TargetOperationOptions`](TargetOperationOptions.md)

#### Returns

`Promise`\<[`RedTeamTargetDetail`](RedTeamTargetDetail.md)\>

***

### deleteAdapter()

> **deleteAdapter**(`uuid`): `Promise`\<`void`\>

Defined in: src/airs/types.ts:722

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<`void`\>

***

### deleteDevices()

> **deleteDevices**(`tenantId`, `serialNumbers`): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: src/airs/types.ts:575

Delete devices by serial numbers.

#### Parameters

##### tenantId

`string`

##### serialNumbers

`string`

#### Returns

`Promise`\<`Record`\<`string`, `unknown`\>\>

***

### deleteInstance()

> **deleteInstance**(`tenantId`): `Promise`\<[`InstanceResponse`](InstanceResponse.md)\>

Defined in: src/airs/types.ts:563

Delete an instance.

#### Parameters

##### tenantId

`string`

#### Returns

`Promise`\<[`InstanceResponse`](InstanceResponse.md)\>

***

### deleteTarget()

> **deleteTarget**(`uuid`): `Promise`\<`void`\>

Defined in: src/airs/types.ts:606

Delete a red team target.

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<`void`\>

***

### getAdapter()

> **getAdapter**(`uuid`): `Promise`\<[`RedTeamAdapterDetail`](RedTeamAdapterDetail.md)\>

Defined in: src/airs/types.ts:711

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<[`RedTeamAdapterDetail`](RedTeamAdapterDetail.md)\>

***

### getCategories()

> **getCategories**(): `Promise`\<[`RedTeamCategory`](RedTeamCategory.md)[]\>

Defined in: src/airs/types.ts:673

List available attack categories.

#### Returns

`Promise`\<[`RedTeamCategory`](RedTeamCategory.md)[]\>

***

### getChannel()

> **getChannel**(`channelId`): `Promise`\<[`RedTeamChannel`](RedTeamChannel.md)\>

Defined in: src/airs/types.ts:688

Get a network broker channel by ID.

#### Parameters

##### channelId

`string`

#### Returns

`Promise`\<[`RedTeamChannel`](RedTeamChannel.md)\>

***

### getChannelStats()

> **getChannelStats**(): `Promise`\<[`RedTeamChannelStats`](RedTeamChannelStats.md)\>

Defined in: src/airs/types.ts:694

Get network broker channel statistics.

#### Returns

`Promise`\<[`RedTeamChannelStats`](RedTeamChannelStats.md)\>

***

### getCustomReport()

> **getCustomReport**(`jobId`): `Promise`\<[`RedTeamCustomReport`](RedTeamCustomReport.md)\>

Defined in: src/airs/types.ts:661

Get custom attack report.

#### Parameters

##### jobId

`string`

#### Returns

`Promise`\<[`RedTeamCustomReport`](RedTeamCustomReport.md)\>

***

### getDynamicReport()

> **getDynamicReport**(`jobId`): `Promise`\<[`RedTeamDynamicReport`](RedTeamDynamicReport.md)\>

Defined in: src/airs/types.ts:658

Get dynamic scan report.

#### Parameters

##### jobId

`string`

#### Returns

`Promise`\<[`RedTeamDynamicReport`](RedTeamDynamicReport.md)\>

***

### getEulaContent()

> **getEulaContent**(): `Promise`\<[`EulaContent`](EulaContent.md)\>

Defined in: src/airs/types.ts:550

Get EULA content.

#### Returns

`Promise`\<[`EulaContent`](EulaContent.md)\>

***

### getEulaStatus()

> **getEulaStatus**(): `Promise`\<[`EulaStatus`](EulaStatus.md)\>

Defined in: src/airs/types.ts:552

Get EULA acceptance status.

#### Returns

`Promise`\<[`EulaStatus`](EulaStatus.md)\>

***

### getInstance()

> **getInstance**(`tenantId`): `Promise`\<[`InstanceDetail`](InstanceDetail.md)\>

Defined in: src/airs/types.ts:559

Get instance details.

#### Parameters

##### tenantId

`string`

#### Returns

`Promise`\<[`InstanceDetail`](InstanceDetail.md)\>

***

### getLanguages()

> **getLanguages**(`management?`): `Promise`\<[`RedTeamLanguages`](RedTeamLanguages.md)\>

Defined in: src/airs/types.ts:697

List tenant languages (data plane, or management plane when `management`).

#### Parameters

##### management?

`boolean`

#### Returns

`Promise`\<[`RedTeamLanguages`](RedTeamLanguages.md)\>

***

### getRegistryCredentials()

> **getRegistryCredentials**(): `Promise`\<[`RegistryCredentials`](RegistryCredentials.md)\>

Defined in: src/airs/types.ts:577

Get or create registry credentials.

#### Returns

`Promise`\<[`RegistryCredentials`](RegistryCredentials.md)\>

***

### getScan()

> **getScan**(`jobId`): `Promise`\<[`RedTeamJob`](RedTeamJob.md)\>

Defined in: src/airs/types.ts:633

Get scan status by job ID.

#### Parameters

##### jobId

`string`

#### Returns

`Promise`\<[`RedTeamJob`](RedTeamJob.md)\>

***

### getStaticReport()

> **getStaticReport**(`jobId`): `Promise`\<[`RedTeamStaticReport`](RedTeamStaticReport.md)\>

Defined in: src/airs/types.ts:655

Get static scan report.

#### Parameters

##### jobId

`string`

#### Returns

`Promise`\<[`RedTeamStaticReport`](RedTeamStaticReport.md)\>

***

### getTarget()

> **getTarget**(`uuid`): `Promise`\<[`RedTeamTargetDetail`](RedTeamTargetDetail.md)\>

Defined in: src/airs/types.ts:590

Get target details.

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<[`RedTeamTargetDetail`](RedTeamTargetDetail.md)\>

***

### getTargetMetadata()

> **getTargetMetadata**(): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: src/airs/types.ts:582

Get target field metadata.

#### Returns

`Promise`\<`Record`\<`string`, `unknown`\>\>

***

### getTargetProfile()

> **getTargetProfile**(`uuid`): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: src/airs/types.ts:612

Get target profile.

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<`Record`\<`string`, `unknown`\>\>

***

### getTargetProfileErrorLogs()

> **getTargetProfileErrorLogs**(`targetId`, `opts?`): `Promise`\<\{ `logs`: [`RedTeamErrorLog`](RedTeamErrorLog.md)[]; `totalItems?`: `number`; \}\>

Defined in: src/airs/types.ts:699

List target-profile error logs.

#### Parameters

##### targetId

`string`

##### opts?

###### limit?

`number`

###### offset?

`number`

###### search?

`string`

#### Returns

`Promise`\<\{ `logs`: [`RedTeamErrorLog`](RedTeamErrorLog.md)[]; `totalItems?`: `number`; \}\>

***

### getTargetTemplates()

> **getTargetTemplates**(): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: src/airs/types.ts:584

Get provider-specific target templates.

#### Returns

`Promise`\<`Record`\<`string`, `unknown`\>\>

***

### listAdapters()

> **listAdapters**(`opts?`): `Promise`\<\{ `adapters`: [`RedTeamAdapterListItem`](RedTeamAdapterListItem.md)[]; `totalItems?`: `number`; \}\>

Defined in: src/airs/types.ts:705

#### Parameters

##### opts?

[`RedTeamAdapterListOptions`](RedTeamAdapterListOptions.md)

#### Returns

`Promise`\<\{ `adapters`: [`RedTeamAdapterListItem`](RedTeamAdapterListItem.md)[]; `totalItems?`: `number`; \}\>

***

### listAllAdapters()

> **listAllAdapters**(`opts?`): `Promise`\<[`RedTeamAdapterListItem`](RedTeamAdapterListItem.md)[]\>

Defined in: src/airs/types.ts:708

#### Parameters

##### opts?

[`RedTeamAdapterListOptions`](RedTeamAdapterListOptions.md) & `object`

#### Returns

`Promise`\<[`RedTeamAdapterListItem`](RedTeamAdapterListItem.md)[]\>

***

### listAllChannels()

> **listAllChannels**(`opts?`): `Promise`\<[`RedTeamChannel`](RedTeamChannel.md)[]\>

Defined in: src/airs/types.ts:686

#### Parameters

##### opts?

[`RedTeamChannelListOptions`](RedTeamChannelListOptions.md) & `object`

#### Returns

`Promise`\<[`RedTeamChannel`](RedTeamChannel.md)[]\>

***

### listAllScans()

> **listAllScans**(`opts?`): `Promise`\<[`RedTeamJob`](RedTeamJob.md)[]\>

Defined in: src/airs/types.ts:643

#### Parameters

##### opts?

###### jobType?

`string`

###### limit?

`number`

###### max?

`number`

###### status?

`string`

###### targetId?

`string`

#### Returns

`Promise`\<[`RedTeamJob`](RedTeamJob.md)[]\>

***

### listAttacks()

> **listAttacks**(`jobId`, `opts?`): `Promise`\<\{ `attacks`: [`RedTeamAttack`](RedTeamAttack.md)[]; `totalItems?`: `number`; \}\>

Defined in: src/airs/types.ts:664

List attacks from a static/dynamic scan.

#### Parameters

##### jobId

`string`

##### opts?

###### limit?

`number`

###### severity?

`string`

#### Returns

`Promise`\<\{ `attacks`: [`RedTeamAttack`](RedTeamAttack.md)[]; `totalItems?`: `number`; \}\>

***

### listChannels()

> **listChannels**(`opts?`): `Promise`\<\{ `channels`: [`RedTeamChannel`](RedTeamChannel.md)[]; `totalItems?`: `number`; \}\>

Defined in: src/airs/types.ts:683

List network broker channels.

#### Parameters

##### opts?

[`RedTeamChannelListOptions`](RedTeamChannelListOptions.md)

#### Returns

`Promise`\<\{ `channels`: [`RedTeamChannel`](RedTeamChannel.md)[]; `totalItems?`: `number`; \}\>

***

### listCustomAttacks()

> **listCustomAttacks**(`jobId`, `opts?`): `Promise`\<[`RedTeamCustomAttack`](RedTeamCustomAttack.md)[]\>

Defined in: src/airs/types.ts:670

List attacks from a custom prompt set scan.

#### Parameters

##### jobId

`string`

##### opts?

###### limit?

`number`

#### Returns

`Promise`\<[`RedTeamCustomAttack`](RedTeamCustomAttack.md)[]\>

***

### listScans()

> **listScans**(`opts?`): `Promise`\<[`RedTeamJob`](RedTeamJob.md)[]\>

Defined in: src/airs/types.ts:636

List recent scans with optional filters.

#### Parameters

##### opts?

###### jobType?

`string`

###### limit?

`number`

###### offset?

`number`

###### status?

`string`

###### targetId?

`string`

#### Returns

`Promise`\<[`RedTeamJob`](RedTeamJob.md)[]\>

***

### listTargets()

> **listTargets**(): `Promise`\<[`RedTeamTarget`](RedTeamTarget.md)[]\>

Defined in: src/airs/types.ts:587

List configured red team targets.

#### Returns

`Promise`\<[`RedTeamTarget`](RedTeamTarget.md)[]\>

***

### probeTarget()

> **probeTarget**(`request`): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: src/airs/types.ts:609

Probe a target connection.

#### Parameters

##### request

`Record`\<`string`, `unknown`\>

#### Returns

`Promise`\<`Record`\<`string`, `unknown`\>\>

***

### updateAdapter()

> **updateAdapter**(`uuid`, `overrides`, `validate?`): `Promise`\<[`RedTeamAdapterDetail`](RedTeamAdapterDetail.md)\>

Defined in: src/airs/types.ts:717

Read-modify-write: merges overrides onto the current record (upstream PUT is full-replacement).

#### Parameters

##### uuid

`string`

##### overrides

[`RedTeamAdapterUpdateOverrides`](RedTeamAdapterUpdateOverrides.md)

##### validate?

`boolean`

#### Returns

`Promise`\<[`RedTeamAdapterDetail`](RedTeamAdapterDetail.md)\>

***

### updateChannel()

> **updateChannel**(`channelId`, `request`): `Promise`\<[`RedTeamChannel`](RedTeamChannel.md)\>

Defined in: src/airs/types.ts:692

Update a network broker channel.

#### Parameters

##### channelId

`string`

##### request

[`RedTeamChannelUpdateRequest`](RedTeamChannelUpdateRequest.md)

#### Returns

`Promise`\<[`RedTeamChannel`](RedTeamChannel.md)\>

***

### updateDevices()

> **updateDevices**(`tenantId`, `request`): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: src/airs/types.ts:570

Update devices (PATCH).

#### Parameters

##### tenantId

`string`

##### request

`Record`\<`string`, `unknown`\>

#### Returns

`Promise`\<`Record`\<`string`, `unknown`\>\>

***

### updateInstance()

> **updateInstance**(`tenantId`, `request`): `Promise`\<[`InstanceResponse`](InstanceResponse.md)\>

Defined in: src/airs/types.ts:561

Update an instance.

#### Parameters

##### tenantId

`string`

##### request

[`InstanceRequest`](InstanceRequest.md)

#### Returns

`Promise`\<[`InstanceResponse`](InstanceResponse.md)\>

***

### updateTarget()

> **updateTarget**(`uuid`, `request`, `opts?`): `Promise`\<[`RedTeamTargetDetail`](RedTeamTargetDetail.md)\>

Defined in: src/airs/types.ts:599

Update a red team target.

#### Parameters

##### uuid

`string`

##### request

[`RedTeamTargetUpdateRequest`](RedTeamTargetUpdateRequest.md)

##### opts?

[`TargetOperationOptions`](TargetOperationOptions.md)

#### Returns

`Promise`\<[`RedTeamTargetDetail`](RedTeamTargetDetail.md)\>

***

### updateTargetProfile()

> **updateTargetProfile**(`uuid`, `request`): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: src/airs/types.ts:615

Update target profile.

#### Parameters

##### uuid

`string`

##### request

`Record`\<`string`, `unknown`\>

#### Returns

`Promise`\<`Record`\<`string`, `unknown`\>\>

***

### validateAdapter()

> **validateAdapter**(`request`): `Promise`\<[`RedTeamAdapterValidationResult`](RedTeamAdapterValidationResult.md)\>

Defined in: src/airs/types.ts:724

Run a script end-to-end through the broker channel; returns an execution outcome.

#### Parameters

##### request

[`RedTeamAdapterValidateRequest`](RedTeamAdapterValidateRequest.md)

#### Returns

`Promise`\<[`RedTeamAdapterValidationResult`](RedTeamAdapterValidationResult.md)\>

***

### validateTargetAuth()

> **validateTargetAuth**(`request`): `Promise`\<[`TargetAuthValidationResult`](TargetAuthValidationResult.md)\>

Defined in: src/airs/types.ts:580

Validate target auth credentials.

#### Parameters

##### request

[`TargetAuthValidationRequest`](TargetAuthValidationRequest.md)

#### Returns

`Promise`\<[`TargetAuthValidationResult`](TargetAuthValidationResult.md)\>

***

### waitForCompletion()

> **waitForCompletion**(`jobId`, `onProgress?`, `intervalMs?`): `Promise`\<[`RedTeamJob`](RedTeamJob.md)\>

Defined in: src/airs/types.ts:676

Poll until scan completes. Calls onProgress for status updates.

#### Parameters

##### jobId

`string`

##### onProgress?

(`job`) => `void`

##### intervalMs?

`number`

#### Returns

`Promise`\<[`RedTeamJob`](RedTeamJob.md)\>
