# Class: SdkRedTeamService

Defined in: src/airs/redteam.ts:234

Contract for AI Red Team scan operations.

## Implements

- [`RedTeamService`](../interfaces/RedTeamService.md)

## Constructors

### Constructor

> **new SdkRedTeamService**(`opts?`): `SdkRedTeamService`

Defined in: src/airs/redteam.ts:237

#### Parameters

##### opts?

`RedTeamClientOptions`

#### Returns

`SdkRedTeamService`

## Methods

### abortScan()

> **abortScan**(`jobId`): `Promise`\<`void`\>

Defined in: src/airs/redteam.ts:520

Abort a running scan.

#### Parameters

##### jobId

`string`

#### Returns

`Promise`\<`void`\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`abortScan`](../interfaces/RedTeamService.md#abortscan)

***

### acceptEula()

> **acceptEula**(`eulaContent`): `Promise`\<[`EulaStatus`](../interfaces/EulaStatus.md)\>

Defined in: src/airs/redteam.ts:255

Accept the EULA.

#### Parameters

##### eulaContent

`string`

#### Returns

`Promise`\<[`EulaStatus`](../interfaces/EulaStatus.md)\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`acceptEula`](../interfaces/RedTeamService.md#accepteula)

***

### createAdapter()

> **createAdapter**(`request`, `validate?`): `Promise`\<[`RedTeamAdapterDetail`](../interfaces/RedTeamAdapterDetail.md)\>

Defined in: src/airs/redteam.ts:815

#### Parameters

##### request

[`RedTeamAdapterCreateRequest`](../interfaces/RedTeamAdapterCreateRequest.md)

##### validate?

`boolean`

#### Returns

`Promise`\<[`RedTeamAdapterDetail`](../interfaces/RedTeamAdapterDetail.md)\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`createAdapter`](../interfaces/RedTeamService.md#createadapter)

***

### createChannel()

> **createChannel**(`request`): `Promise`\<[`RedTeamChannel`](../interfaces/RedTeamChannel.md)\>

Defined in: src/airs/redteam.ts:709

Create a network broker channel.

#### Parameters

##### request

[`RedTeamChannelCreateRequest`](../interfaces/RedTeamChannelCreateRequest.md)

#### Returns

`Promise`\<[`RedTeamChannel`](../interfaces/RedTeamChannel.md)\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`createChannel`](../interfaces/RedTeamService.md#createchannel)

***

### createDevices()

> **createDevices**(`tenantId`, `request`): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: src/airs/redteam.ts:317

Create devices for an instance.

#### Parameters

##### tenantId

`string`

##### request

`Record`\<`string`, `unknown`\>

#### Returns

`Promise`\<`Record`\<`string`, `unknown`\>\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`createDevices`](../interfaces/RedTeamService.md#createdevices)

***

### createInstance()

> **createInstance**(`request`): `Promise`\<[`InstanceResponse`](../interfaces/InstanceResponse.md)\>

Defined in: src/airs/redteam.ts:267

Create an instance.

#### Parameters

##### request

[`InstanceRequest`](../interfaces/InstanceRequest.md)

#### Returns

`Promise`\<[`InstanceResponse`](../interfaces/InstanceResponse.md)\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`createInstance`](../interfaces/RedTeamService.md#createinstance)

***

### createScan()

> **createScan**(`request`): `Promise`\<[`RedTeamJob`](../interfaces/RedTeamJob.md)\>

Defined in: src/airs/redteam.ts:439

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

`Promise`\<[`RedTeamJob`](../interfaces/RedTeamJob.md)\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`createScan`](../interfaces/RedTeamService.md#createscan)

***

### createTarget()

> **createTarget**(`request`, `opts?`): `Promise`\<[`RedTeamTargetDetail`](../interfaces/RedTeamTargetDetail.md)\>

Defined in: src/airs/redteam.ts:395

Create a red team target.

#### Parameters

##### request

[`RedTeamTargetCreateRequest`](../interfaces/RedTeamTargetCreateRequest.md)

##### opts?

[`TargetOperationOptions`](../interfaces/TargetOperationOptions.md)

#### Returns

`Promise`\<[`RedTeamTargetDetail`](../interfaces/RedTeamTargetDetail.md)\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`createTarget`](../interfaces/RedTeamService.md#createtarget)

***

### deleteAdapter()

> **deleteAdapter**(`uuid`): `Promise`\<`void`\>

Defined in: src/airs/redteam.ts:869

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<`void`\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`deleteAdapter`](../interfaces/RedTeamService.md#deleteadapter)

***

### deleteDevices()

> **deleteDevices**(`tenantId`, `serialNumbers`): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: src/airs/redteam.ts:337

Delete devices by serial numbers.

#### Parameters

##### tenantId

`string`

##### serialNumbers

`string`

#### Returns

`Promise`\<`Record`\<`string`, `unknown`\>\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`deleteDevices`](../interfaces/RedTeamService.md#deletedevices)

***

### deleteInstance()

> **deleteInstance**(`tenantId`): `Promise`\<[`InstanceResponse`](../interfaces/InstanceResponse.md)\>

Defined in: src/airs/redteam.ts:307

Delete an instance.

#### Parameters

##### tenantId

`string`

#### Returns

`Promise`\<[`InstanceResponse`](../interfaces/InstanceResponse.md)\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`deleteInstance`](../interfaces/RedTeamService.md#deleteinstance)

***

### deleteTarget()

> **deleteTarget**(`uuid`): `Promise`\<`void`\>

Defined in: src/airs/redteam.ts:412

Delete a red team target.

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<`void`\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`deleteTarget`](../interfaces/RedTeamService.md#deletetarget)

***

### getAdapter()

> **getAdapter**(`uuid`): `Promise`\<[`RedTeamAdapterDetail`](../interfaces/RedTeamAdapterDetail.md)\>

Defined in: src/airs/redteam.ts:810

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<[`RedTeamAdapterDetail`](../interfaces/RedTeamAdapterDetail.md)\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getAdapter`](../interfaces/RedTeamService.md#getadapter)

***

### getCategories()

> **getCategories**(): `Promise`\<[`RedTeamCategory`](../interfaces/RedTeamCategory.md)[]\>

Defined in: src/airs/redteam.ts:633

List available attack categories.

#### Returns

`Promise`\<[`RedTeamCategory`](../interfaces/RedTeamCategory.md)[]\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getCategories`](../interfaces/RedTeamService.md#getcategories)

***

### getChannel()

> **getChannel**(`channelId`): `Promise`\<[`RedTeamChannel`](../interfaces/RedTeamChannel.md)\>

Defined in: src/airs/redteam.ts:704

Get a network broker channel by ID.

#### Parameters

##### channelId

`string`

#### Returns

`Promise`\<[`RedTeamChannel`](../interfaces/RedTeamChannel.md)\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getChannel`](../interfaces/RedTeamService.md#getchannel)

***

### getChannelStats()

> **getChannelStats**(): `Promise`\<[`RedTeamChannelStats`](../interfaces/RedTeamChannelStats.md)\>

Defined in: src/airs/redteam.ts:732

Get network broker channel statistics.

#### Returns

`Promise`\<[`RedTeamChannelStats`](../interfaces/RedTeamChannelStats.md)\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getChannelStats`](../interfaces/RedTeamService.md#getchannelstats)

***

### getCustomReport()

> **getCustomReport**(`jobId`): `Promise`\<[`RedTeamCustomReport`](../interfaces/RedTeamCustomReport.md)\>

Defined in: src/airs/redteam.ts:570

Get custom attack report.

#### Parameters

##### jobId

`string`

#### Returns

`Promise`\<[`RedTeamCustomReport`](../interfaces/RedTeamCustomReport.md)\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getCustomReport`](../interfaces/RedTeamService.md#getcustomreport)

***

### getDynamicReport()

> **getDynamicReport**(`jobId`): `Promise`\<[`RedTeamDynamicReport`](../interfaces/RedTeamDynamicReport.md)\>

Defined in: src/airs/redteam.ts:557

Get dynamic scan report.

#### Parameters

##### jobId

`string`

#### Returns

`Promise`\<[`RedTeamDynamicReport`](../interfaces/RedTeamDynamicReport.md)\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getDynamicReport`](../interfaces/RedTeamService.md#getdynamicreport)

***

### getEulaContent()

> **getEulaContent**(): `Promise`\<[`EulaContent`](../interfaces/EulaContent.md)\>

Defined in: src/airs/redteam.ts:241

Get EULA content.

#### Returns

`Promise`\<[`EulaContent`](../interfaces/EulaContent.md)\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getEulaContent`](../interfaces/RedTeamService.md#geteulacontent)

***

### getEulaStatus()

> **getEulaStatus**(): `Promise`\<[`EulaStatus`](../interfaces/EulaStatus.md)\>

Defined in: src/airs/redteam.ts:246

Get EULA acceptance status.

#### Returns

`Promise`\<[`EulaStatus`](../interfaces/EulaStatus.md)\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getEulaStatus`](../interfaces/RedTeamService.md#geteulastatus)

***

### getInstance()

> **getInstance**(`tenantId`): `Promise`\<[`InstanceDetail`](../interfaces/InstanceDetail.md)\>

Defined in: src/airs/redteam.ts:282

Get instance details.

#### Parameters

##### tenantId

`string`

#### Returns

`Promise`\<[`InstanceDetail`](../interfaces/InstanceDetail.md)\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getInstance`](../interfaces/RedTeamService.md#getinstance)

***

### getLanguages()

> **getLanguages**(`management?`): `Promise`\<[`RedTeamLanguages`](../interfaces/RedTeamLanguages.md)\>

Defined in: src/airs/redteam.ts:745

List tenant languages (data plane, or management plane when `management`).

#### Parameters

##### management?

`boolean` = `false`

#### Returns

`Promise`\<[`RedTeamLanguages`](../interfaces/RedTeamLanguages.md)\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getLanguages`](../interfaces/RedTeamService.md#getlanguages)

***

### getRegistryCredentials()

> **getRegistryCredentials**(): `Promise`\<[`RegistryCredentials`](../interfaces/RegistryCredentials.md)\>

Defined in: src/airs/redteam.ts:344

Get or create registry credentials.

#### Returns

`Promise`\<[`RegistryCredentials`](../interfaces/RegistryCredentials.md)\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getRegistryCredentials`](../interfaces/RedTeamService.md#getregistrycredentials)

***

### getScan()

> **getScan**(`jobId`): `Promise`\<[`RedTeamJob`](../interfaces/RedTeamJob.md)\>

Defined in: src/airs/redteam.ts:478

Get scan status by job ID.

#### Parameters

##### jobId

`string`

#### Returns

`Promise`\<[`RedTeamJob`](../interfaces/RedTeamJob.md)\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getScan`](../interfaces/RedTeamService.md#getscan)

***

### getStaticReport()

> **getStaticReport**(`jobId`): `Promise`\<[`RedTeamStaticReport`](../interfaces/RedTeamStaticReport.md)\>

Defined in: src/airs/redteam.ts:524

Get static scan report.

#### Parameters

##### jobId

`string`

#### Returns

`Promise`\<[`RedTeamStaticReport`](../interfaces/RedTeamStaticReport.md)\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getStaticReport`](../interfaces/RedTeamService.md#getstaticreport)

***

### getTarget()

> **getTarget**(`uuid`): `Promise`\<[`RedTeamTargetDetail`](../interfaces/RedTeamTargetDetail.md)\>

Defined in: src/airs/redteam.ts:390

Get target details.

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<[`RedTeamTargetDetail`](../interfaces/RedTeamTargetDetail.md)\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getTarget`](../interfaces/RedTeamService.md#gettarget)

***

### getTargetMetadata()

> **getTargetMetadata**(): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: src/airs/redteam.ts:371

Get target field metadata.

#### Returns

`Promise`\<`Record`\<`string`, `unknown`\>\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getTargetMetadata`](../interfaces/RedTeamService.md#gettargetmetadata)

***

### getTargetProfile()

> **getTargetProfile**(`uuid`): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: src/airs/redteam.ts:426

Get target profile.

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<`Record`\<`string`, `unknown`\>\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getTargetProfile`](../interfaces/RedTeamService.md#gettargetprofile)

***

### getTargetProfileErrorLogs()

> **getTargetProfileErrorLogs**(`targetId`, `opts?`): `Promise`\<\{ `logs`: [`RedTeamErrorLog`](../interfaces/RedTeamErrorLog.md)[]; `totalItems?`: `number`; \}\>

Defined in: src/airs/redteam.ts:759

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

`Promise`\<\{ `logs`: [`RedTeamErrorLog`](../interfaces/RedTeamErrorLog.md)[]; `totalItems?`: `number`; \}\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getTargetProfileErrorLogs`](../interfaces/RedTeamService.md#gettargetprofileerrorlogs)

***

### getTargetTemplates()

> **getTargetTemplates**(): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: src/airs/redteam.ts:375

Get provider-specific target templates.

#### Returns

`Promise`\<`Record`\<`string`, `unknown`\>\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getTargetTemplates`](../interfaces/RedTeamService.md#gettargettemplates)

***

### listAdapters()

> **listAdapters**(`opts?`): `Promise`\<\{ `adapters`: [`RedTeamAdapterListItem`](../interfaces/RedTeamAdapterListItem.md)[]; `totalItems?`: `number`; \}\>

Defined in: src/airs/redteam.ts:783

#### Parameters

##### opts?

[`RedTeamAdapterListOptions`](../interfaces/RedTeamAdapterListOptions.md)

#### Returns

`Promise`\<\{ `adapters`: [`RedTeamAdapterListItem`](../interfaces/RedTeamAdapterListItem.md)[]; `totalItems?`: `number`; \}\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`listAdapters`](../interfaces/RedTeamService.md#listadapters)

***

### listAllAdapters()

> **listAllAdapters**(`opts?`): `Promise`\<[`RedTeamAdapterListItem`](../interfaces/RedTeamAdapterListItem.md)[]\>

Defined in: src/airs/redteam.ts:799

#### Parameters

##### opts?

[`RedTeamAdapterListOptions`](../interfaces/RedTeamAdapterListOptions.md) & `object` = `{}`

#### Returns

`Promise`\<[`RedTeamAdapterListItem`](../interfaces/RedTeamAdapterListItem.md)[]\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`listAllAdapters`](../interfaces/RedTeamService.md#listalladapters)

***

### listAllChannels()

> **listAllChannels**(`opts?`): `Promise`\<[`RedTeamChannel`](../interfaces/RedTeamChannel.md)[]\>

Defined in: src/airs/redteam.ts:686

#### Parameters

##### opts?

[`RedTeamChannelListOptions`](../interfaces/RedTeamChannelListOptions.md) & `object` = `{}`

#### Returns

`Promise`\<[`RedTeamChannel`](../interfaces/RedTeamChannel.md)[]\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`listAllChannels`](../interfaces/RedTeamService.md#listallchannels)

***

### listAllScans()

> **listAllScans**(`opts?`): `Promise`\<[`RedTeamJob`](../interfaces/RedTeamJob.md)[]\>

Defined in: src/airs/redteam.ts:503

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

`Promise`\<[`RedTeamJob`](../interfaces/RedTeamJob.md)[]\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`listAllScans`](../interfaces/RedTeamService.md#listallscans)

***

### listAttacks()

> **listAttacks**(`jobId`, `opts?`): `Promise`\<\{ `attacks`: [`RedTeamAttack`](../interfaces/RedTeamAttack.md)[]; `totalItems?`: `number`; \}\>

Defined in: src/airs/redteam.ts:592

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

`Promise`\<\{ `attacks`: [`RedTeamAttack`](../interfaces/RedTeamAttack.md)[]; `totalItems?`: `number`; \}\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`listAttacks`](../interfaces/RedTeamService.md#listattacks)

***

### listChannels()

> **listChannels**(`opts?`): `Promise`\<\{ `channels`: [`RedTeamChannel`](../interfaces/RedTeamChannel.md)[]; `totalItems?`: `number`; \}\>

Defined in: src/airs/redteam.ts:669

List network broker channels.

#### Parameters

##### opts?

[`RedTeamChannelListOptions`](../interfaces/RedTeamChannelListOptions.md)

#### Returns

`Promise`\<\{ `channels`: [`RedTeamChannel`](../interfaces/RedTeamChannel.md)[]; `totalItems?`: `number`; \}\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`listChannels`](../interfaces/RedTeamService.md#listchannels)

***

### listCustomAttacks()

> **listCustomAttacks**(`jobId`, `opts?`): `Promise`\<[`RedTeamCustomAttack`](../interfaces/RedTeamCustomAttack.md)[]\>

Defined in: src/airs/redteam.ts:616

List attacks from a custom prompt set scan.

#### Parameters

##### jobId

`string`

##### opts?

###### limit?

`number`

#### Returns

`Promise`\<[`RedTeamCustomAttack`](../interfaces/RedTeamCustomAttack.md)[]\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`listCustomAttacks`](../interfaces/RedTeamService.md#listcustomattacks)

***

### listScans()

> **listScans**(`opts?`): `Promise`\<[`RedTeamJob`](../interfaces/RedTeamJob.md)[]\>

Defined in: src/airs/redteam.ts:483

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

`Promise`\<[`RedTeamJob`](../interfaces/RedTeamJob.md)[]\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`listScans`](../interfaces/RedTeamService.md#listscans)

***

### listTargets()

> **listTargets**(): `Promise`\<[`RedTeamTarget`](../interfaces/RedTeamTarget.md)[]\>

Defined in: src/airs/redteam.ts:379

List configured red team targets.

#### Returns

`Promise`\<[`RedTeamTarget`](../interfaces/RedTeamTarget.md)[]\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`listTargets`](../interfaces/RedTeamService.md#listtargets)

***

### probeTarget()

> **probeTarget**(`request`): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: src/airs/redteam.ts:416

Probe a target connection.

#### Parameters

##### request

`Record`\<`string`, `unknown`\>

#### Returns

`Promise`\<`Record`\<`string`, `unknown`\>\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`probeTarget`](../interfaces/RedTeamService.md#probetarget)

***

### updateAdapter()

> **updateAdapter**(`uuid`, `overrides`, `validate?`): `Promise`\<[`RedTeamAdapterDetail`](../interfaces/RedTeamAdapterDetail.md)\>

Defined in: src/airs/redteam.ts:838

Read-modify-write: merges overrides onto the current record (upstream PUT is full-replacement).

#### Parameters

##### uuid

`string`

##### overrides

[`RedTeamAdapterUpdateOverrides`](../interfaces/RedTeamAdapterUpdateOverrides.md)

##### validate?

`boolean`

#### Returns

`Promise`\<[`RedTeamAdapterDetail`](../interfaces/RedTeamAdapterDetail.md)\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`updateAdapter`](../interfaces/RedTeamService.md#updateadapter)

***

### updateChannel()

> **updateChannel**(`channelId`, `request`): `Promise`\<[`RedTeamChannel`](../interfaces/RedTeamChannel.md)\>

Defined in: src/airs/redteam.ts:718

Update a network broker channel.

#### Parameters

##### channelId

`string`

##### request

[`RedTeamChannelUpdateRequest`](../interfaces/RedTeamChannelUpdateRequest.md)

#### Returns

`Promise`\<[`RedTeamChannel`](../interfaces/RedTeamChannel.md)\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`updateChannel`](../interfaces/RedTeamService.md#updatechannel)

***

### updateDevices()

> **updateDevices**(`tenantId`, `request`): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: src/airs/redteam.ts:327

Update devices (PATCH).

#### Parameters

##### tenantId

`string`

##### request

`Record`\<`string`, `unknown`\>

#### Returns

`Promise`\<`Record`\<`string`, `unknown`\>\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`updateDevices`](../interfaces/RedTeamService.md#updatedevices)

***

### updateInstance()

> **updateInstance**(`tenantId`, `request`): `Promise`\<[`InstanceResponse`](../interfaces/InstanceResponse.md)\>

Defined in: src/airs/redteam.ts:292

Update an instance.

#### Parameters

##### tenantId

`string`

##### request

[`InstanceRequest`](../interfaces/InstanceRequest.md)

#### Returns

`Promise`\<[`InstanceResponse`](../interfaces/InstanceResponse.md)\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`updateInstance`](../interfaces/RedTeamService.md#updateinstance)

***

### updateTarget()

> **updateTarget**(`uuid`, `request`, `opts?`): `Promise`\<[`RedTeamTargetDetail`](../interfaces/RedTeamTargetDetail.md)\>

Defined in: src/airs/redteam.ts:403

Update a red team target.

#### Parameters

##### uuid

`string`

##### request

[`RedTeamTargetUpdateRequest`](../interfaces/RedTeamTargetUpdateRequest.md)

##### opts?

[`TargetOperationOptions`](../interfaces/TargetOperationOptions.md)

#### Returns

`Promise`\<[`RedTeamTargetDetail`](../interfaces/RedTeamTargetDetail.md)\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`updateTarget`](../interfaces/RedTeamService.md#updatetarget)

***

### updateTargetProfile()

> **updateTargetProfile**(`uuid`, `request`): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: src/airs/redteam.ts:431

Update target profile.

#### Parameters

##### uuid

`string`

##### request

`Record`\<`string`, `unknown`\>

#### Returns

`Promise`\<`Record`\<`string`, `unknown`\>\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`updateTargetProfile`](../interfaces/RedTeamService.md#updatetargetprofile)

***

### validateAdapter()

> **validateAdapter**(`request`): `Promise`\<[`RedTeamAdapterValidationResult`](../interfaces/RedTeamAdapterValidationResult.md)\>

Defined in: src/airs/redteam.ts:873

Run a script end-to-end through the broker channel; returns an execution outcome.

#### Parameters

##### request

[`RedTeamAdapterValidateRequest`](../interfaces/RedTeamAdapterValidateRequest.md)

#### Returns

`Promise`\<[`RedTeamAdapterValidationResult`](../interfaces/RedTeamAdapterValidationResult.md)\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`validateAdapter`](../interfaces/RedTeamService.md#validateadapter)

***

### validateTargetAuth()

> **validateTargetAuth**(`request`): `Promise`\<[`TargetAuthValidationResult`](../interfaces/TargetAuthValidationResult.md)\>

Defined in: src/airs/redteam.ts:352

Validate target auth credentials.

#### Parameters

##### request

[`TargetAuthValidationRequest`](../interfaces/TargetAuthValidationRequest.md)

#### Returns

`Promise`\<[`TargetAuthValidationResult`](../interfaces/TargetAuthValidationResult.md)\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`validateTargetAuth`](../interfaces/RedTeamService.md#validatetargetauth)

***

### waitForCompletion()

> **waitForCompletion**(`jobId`, `onProgress?`, `intervalMs?`): `Promise`\<[`RedTeamJob`](../interfaces/RedTeamJob.md)\>

Defined in: src/airs/redteam.ts:647

Poll until scan completes. Calls onProgress for status updates.

#### Parameters

##### jobId

`string`

##### onProgress?

(`job`) => `void`

##### intervalMs?

`number` = `5000`

#### Returns

`Promise`\<[`RedTeamJob`](../interfaces/RedTeamJob.md)\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`waitForCompletion`](../interfaces/RedTeamService.md#waitforcompletion)
