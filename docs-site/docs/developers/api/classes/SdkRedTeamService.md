# Class: SdkRedTeamService

Defined in: [src/airs/redteam.ts:234](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L234)

Contract for AI Red Team scan operations.

## Implements

- [`RedTeamService`](../interfaces/RedTeamService.md)

## Constructors

### Constructor

> **new SdkRedTeamService**(`opts?`): `SdkRedTeamService`

Defined in: [src/airs/redteam.ts:237](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L237)

#### Parameters

##### opts?

`RedTeamClientOptions`

#### Returns

`SdkRedTeamService`

## Methods

### abortScan()

> **abortScan**(`jobId`): `Promise`\<`void`\>

Defined in: [src/airs/redteam.ts:513](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L513)

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

> **acceptEula**(`eulaContent`): `Promise`\<`EulaStatus`\>

Defined in: [src/airs/redteam.ts:255](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L255)

Accept the EULA.

#### Parameters

##### eulaContent

`string`

#### Returns

`Promise`\<`EulaStatus`\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`acceptEula`](../interfaces/RedTeamService.md#accepteula)

***

### createAdapter()

> **createAdapter**(`request`, `validate?`): `Promise`\<`RedTeamAdapterDetail`\>

Defined in: [src/airs/redteam.ts:779](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L779)

#### Parameters

##### request

`RedTeamAdapterCreateRequest`

##### validate?

`boolean`

#### Returns

`Promise`\<`RedTeamAdapterDetail`\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`createAdapter`](../interfaces/RedTeamService.md#createadapter)

***

### createChannel()

> **createChannel**(`request`): `Promise`\<`RedTeamChannel`\>

Defined in: [src/airs/redteam.ts:684](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L684)

Create a network broker channel.

#### Parameters

##### request

`RedTeamChannelCreateRequest`

#### Returns

`Promise`\<`RedTeamChannel`\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`createChannel`](../interfaces/RedTeamService.md#createchannel)

***

### createDevices()

> **createDevices**(`tenantId`, `request`): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: [src/airs/redteam.ts:317](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L317)

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

> **createInstance**(`request`): `Promise`\<`InstanceResponse`\>

Defined in: [src/airs/redteam.ts:267](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L267)

Create an instance.

#### Parameters

##### request

`InstanceRequest`

#### Returns

`Promise`\<`InstanceResponse`\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`createInstance`](../interfaces/RedTeamService.md#createinstance)

***

### createScan()

> **createScan**(`request`): `Promise`\<[`RedTeamJob`](../interfaces/RedTeamJob.md)\>

Defined in: [src/airs/redteam.ts:456](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L456)

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

Defined in: [src/airs/redteam.ts:412](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L412)

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

Defined in: [src/airs/redteam.ts:833](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L833)

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

Defined in: [src/airs/redteam.ts:337](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L337)

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

> **deleteInstance**(`tenantId`): `Promise`\<`InstanceResponse`\>

Defined in: [src/airs/redteam.ts:307](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L307)

Delete an instance.

#### Parameters

##### tenantId

`string`

#### Returns

`Promise`\<`InstanceResponse`\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`deleteInstance`](../interfaces/RedTeamService.md#deleteinstance)

***

### deleteTarget()

> **deleteTarget**(`uuid`): `Promise`\<`void`\>

Defined in: [src/airs/redteam.ts:429](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L429)

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

> **getAdapter**(`uuid`): `Promise`\<`RedTeamAdapterDetail`\>

Defined in: [src/airs/redteam.ts:774](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L774)

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<`RedTeamAdapterDetail`\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getAdapter`](../interfaces/RedTeamService.md#getadapter)

***

### getCategories()

> **getCategories**(): `Promise`\<[`RedTeamCategory`](../interfaces/RedTeamCategory.md)[]\>

Defined in: [src/airs/redteam.ts:626](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L626)

List available attack categories.

#### Returns

`Promise`\<[`RedTeamCategory`](../interfaces/RedTeamCategory.md)[]\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getCategories`](../interfaces/RedTeamService.md#getcategories)

***

### getChannel()

> **getChannel**(`channelId`): `Promise`\<`RedTeamChannel`\>

Defined in: [src/airs/redteam.ts:679](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L679)

Get a network broker channel by ID.

#### Parameters

##### channelId

`string`

#### Returns

`Promise`\<`RedTeamChannel`\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getChannel`](../interfaces/RedTeamService.md#getchannel)

***

### getChannelStats()

> **getChannelStats**(): `Promise`\<`RedTeamChannelStats`\>

Defined in: [src/airs/redteam.ts:707](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L707)

Get network broker channel statistics.

#### Returns

`Promise`\<`RedTeamChannelStats`\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getChannelStats`](../interfaces/RedTeamService.md#getchannelstats)

***

### getCustomReport()

> **getCustomReport**(`jobId`): `Promise`\<[`RedTeamCustomReport`](../interfaces/RedTeamCustomReport.md)\>

Defined in: [src/airs/redteam.ts:563](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L563)

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

> **getDynamicReport**(`jobId`): `Promise`\<`RedTeamDynamicReport`\>

Defined in: [src/airs/redteam.ts:550](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L550)

Get dynamic scan report.

#### Parameters

##### jobId

`string`

#### Returns

`Promise`\<`RedTeamDynamicReport`\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getDynamicReport`](../interfaces/RedTeamService.md#getdynamicreport)

***

### getEulaContent()

> **getEulaContent**(): `Promise`\<`EulaContent`\>

Defined in: [src/airs/redteam.ts:241](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L241)

Get EULA content.

#### Returns

`Promise`\<`EulaContent`\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getEulaContent`](../interfaces/RedTeamService.md#geteulacontent)

***

### getEulaStatus()

> **getEulaStatus**(): `Promise`\<`EulaStatus`\>

Defined in: [src/airs/redteam.ts:246](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L246)

Get EULA acceptance status.

#### Returns

`Promise`\<`EulaStatus`\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getEulaStatus`](../interfaces/RedTeamService.md#geteulastatus)

***

### getInstance()

> **getInstance**(`tenantId`): `Promise`\<`InstanceDetail`\>

Defined in: [src/airs/redteam.ts:282](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L282)

Get instance details.

#### Parameters

##### tenantId

`string`

#### Returns

`Promise`\<`InstanceDetail`\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getInstance`](../interfaces/RedTeamService.md#getinstance)

***

### getLanguages()

> **getLanguages**(`management?`): `Promise`\<`RedTeamLanguages`\>

Defined in: [src/airs/redteam.ts:720](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L720)

List tenant languages (data plane, or management plane when `management`).

#### Parameters

##### management?

`boolean` = `false`

#### Returns

`Promise`\<`RedTeamLanguages`\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getLanguages`](../interfaces/RedTeamService.md#getlanguages)

***

### getRegistryCredentials()

> **getRegistryCredentials**(): `Promise`\<`RegistryCredentials`\>

Defined in: [src/airs/redteam.ts:344](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L344)

Get or create registry credentials.

#### Returns

`Promise`\<`RegistryCredentials`\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getRegistryCredentials`](../interfaces/RedTeamService.md#getregistrycredentials)

***

### getScan()

> **getScan**(`jobId`): `Promise`\<[`RedTeamJob`](../interfaces/RedTeamJob.md)\>

Defined in: [src/airs/redteam.ts:490](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L490)

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

Defined in: [src/airs/redteam.ts:517](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L517)

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

Defined in: [src/airs/redteam.ts:407](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L407)

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

Defined in: [src/airs/redteam.ts:371](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L371)

Get target field metadata.

#### Returns

`Promise`\<`Record`\<`string`, `unknown`\>\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getTargetMetadata`](../interfaces/RedTeamService.md#gettargetmetadata)

***

### getTargetProfile()

> **getTargetProfile**(`uuid`): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: [src/airs/redteam.ts:443](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L443)

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

> **getTargetProfileErrorLogs**(`targetId`, `opts?`): `Promise`\<\{ `logs`: `RedTeamErrorLog`[]; `totalItems?`: `number`; \}\>

Defined in: [src/airs/redteam.ts:734](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L734)

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

`Promise`\<\{ `logs`: `RedTeamErrorLog`[]; `totalItems?`: `number`; \}\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getTargetProfileErrorLogs`](../interfaces/RedTeamService.md#gettargetprofileerrorlogs)

***

### getTargetTemplates()

> **getTargetTemplates**(): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: [src/airs/redteam.ts:375](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L375)

Get provider-specific target templates.

#### Returns

`Promise`\<`Record`\<`string`, `unknown`\>\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getTargetTemplates`](../interfaces/RedTeamService.md#gettargettemplates)

***

### listAdapters()

> **listAdapters**(`opts?`): `Promise`\<\{ `adapters`: `RedTeamAdapterListItem`[]; `totalItems?`: `number`; \}\>

Defined in: [src/airs/redteam.ts:758](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L758)

#### Parameters

##### opts?

`RedTeamAdapterListOptions`

#### Returns

`Promise`\<\{ `adapters`: `RedTeamAdapterListItem`[]; `totalItems?`: `number`; \}\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`listAdapters`](../interfaces/RedTeamService.md#listadapters)

***

### listAttacks()

> **listAttacks**(`jobId`, `opts?`): `Promise`\<\{ `attacks`: [`RedTeamAttack`](../interfaces/RedTeamAttack.md)[]; `totalItems?`: `number`; \}\>

Defined in: [src/airs/redteam.ts:585](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L585)

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

> **listChannels**(`opts?`): `Promise`\<\{ `channels`: `RedTeamChannel`[]; `totalItems?`: `number`; \}\>

Defined in: [src/airs/redteam.ts:662](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L662)

List network broker channels.

#### Parameters

##### opts?

`RedTeamChannelListOptions`

#### Returns

`Promise`\<\{ `channels`: `RedTeamChannel`[]; `totalItems?`: `number`; \}\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`listChannels`](../interfaces/RedTeamService.md#listchannels)

***

### listCustomAttacks()

> **listCustomAttacks**(`jobId`, `opts?`): `Promise`\<[`RedTeamCustomAttack`](../interfaces/RedTeamCustomAttack.md)[]\>

Defined in: [src/airs/redteam.ts:609](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L609)

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

Defined in: [src/airs/redteam.ts:495](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L495)

List recent scans with optional filters.

#### Parameters

##### opts?

###### jobType?

`string`

###### limit?

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

Defined in: [src/airs/redteam.ts:379](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L379)

List configured red team targets.

#### Returns

`Promise`\<[`RedTeamTarget`](../interfaces/RedTeamTarget.md)[]\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`listTargets`](../interfaces/RedTeamService.md#listtargets)

***

### probeTarget()

> **probeTarget**(`request`): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: [src/airs/redteam.ts:433](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L433)

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

> **updateAdapter**(`uuid`, `overrides`, `validate?`): `Promise`\<`RedTeamAdapterDetail`\>

Defined in: [src/airs/redteam.ts:802](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L802)

Read-modify-write: merges overrides onto the current record (upstream PUT is full-replacement).

#### Parameters

##### uuid

`string`

##### overrides

`RedTeamAdapterUpdateOverrides`

##### validate?

`boolean`

#### Returns

`Promise`\<`RedTeamAdapterDetail`\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`updateAdapter`](../interfaces/RedTeamService.md#updateadapter)

***

### updateChannel()

> **updateChannel**(`channelId`, `request`): `Promise`\<`RedTeamChannel`\>

Defined in: [src/airs/redteam.ts:693](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L693)

Update a network broker channel.

#### Parameters

##### channelId

`string`

##### request

`RedTeamChannelUpdateRequest`

#### Returns

`Promise`\<`RedTeamChannel`\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`updateChannel`](../interfaces/RedTeamService.md#updatechannel)

***

### updateDevices()

> **updateDevices**(`tenantId`, `request`): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: [src/airs/redteam.ts:327](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L327)

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

> **updateInstance**(`tenantId`, `request`): `Promise`\<`InstanceResponse`\>

Defined in: [src/airs/redteam.ts:292](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L292)

Update an instance.

#### Parameters

##### tenantId

`string`

##### request

`InstanceRequest`

#### Returns

`Promise`\<`InstanceResponse`\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`updateInstance`](../interfaces/RedTeamService.md#updateinstance)

***

### updateTarget()

> **updateTarget**(`uuid`, `request`, `opts?`): `Promise`\<[`RedTeamTargetDetail`](../interfaces/RedTeamTargetDetail.md)\>

Defined in: [src/airs/redteam.ts:420](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L420)

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

Defined in: [src/airs/redteam.ts:448](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L448)

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

> **validateAdapter**(`request`): `Promise`\<`RedTeamAdapterValidationResult`\>

Defined in: [src/airs/redteam.ts:837](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L837)

Run a script end-to-end through the broker channel; returns an execution outcome.

#### Parameters

##### request

`RedTeamAdapterValidateRequest`

#### Returns

`Promise`\<`RedTeamAdapterValidationResult`\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`validateAdapter`](../interfaces/RedTeamService.md#validateadapter)

***

### validateTargetAuth()

> **validateTargetAuth**(`request`): `Promise`\<`TargetAuthValidationResult`\>

Defined in: [src/airs/redteam.ts:352](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L352)

Validate target auth credentials.

#### Parameters

##### request

`TargetAuthValidationRequest`

#### Returns

`Promise`\<`TargetAuthValidationResult`\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`validateTargetAuth`](../interfaces/RedTeamService.md#validatetargetauth)

***

### waitForCompletion()

> **waitForCompletion**(`jobId`, `onProgress?`, `intervalMs?`): `Promise`\<[`RedTeamJob`](../interfaces/RedTeamJob.md)\>

Defined in: [src/airs/redteam.ts:640](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L640)

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
