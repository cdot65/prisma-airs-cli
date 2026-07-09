# Class: SdkRedTeamService

Defined in: [src/airs/redteam.ts:159](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L159)

Wraps the SDK's RedTeamClient to implement RedTeamService.
Provides scan creation, status polling, report retrieval, and target/category listing.

## Implements

- [`RedTeamService`](../interfaces/RedTeamService.md)

## Constructors

### Constructor

> **new SdkRedTeamService**(`opts?`): `SdkRedTeamService`

Defined in: [src/airs/redteam.ts:162](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L162)

#### Parameters

##### opts?

`RedTeamClientOptions`

#### Returns

`SdkRedTeamService`

## Methods

### abortScan()

> **abortScan**(`jobId`): `Promise`\<`void`\>

Defined in: [src/airs/redteam.ts:438](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L438)

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

Defined in: [src/airs/redteam.ts:180](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L180)

Accept the EULA.

#### Parameters

##### eulaContent

`string`

#### Returns

`Promise`\<`EulaStatus`\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`acceptEula`](../interfaces/RedTeamService.md#accepteula)

***

### createChannel()

> **createChannel**(`request`): `Promise`\<`RedTeamChannel`\>

Defined in: [src/airs/redteam.ts:609](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L609)

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

Defined in: [src/airs/redteam.ts:242](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L242)

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

Defined in: [src/airs/redteam.ts:192](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L192)

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

Defined in: [src/airs/redteam.ts:381](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L381)

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

Defined in: [src/airs/redteam.ts:337](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L337)

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

### deleteDevices()

> **deleteDevices**(`tenantId`, `serialNumbers`): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: [src/airs/redteam.ts:262](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L262)

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

Defined in: [src/airs/redteam.ts:232](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L232)

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

Defined in: [src/airs/redteam.ts:354](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L354)

Delete a red team target.

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<`void`\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`deleteTarget`](../interfaces/RedTeamService.md#deletetarget)

***

### getCategories()

> **getCategories**(): `Promise`\<[`RedTeamCategory`](../interfaces/RedTeamCategory.md)[]\>

Defined in: [src/airs/redteam.ts:551](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L551)

List available attack categories.

#### Returns

`Promise`\<[`RedTeamCategory`](../interfaces/RedTeamCategory.md)[]\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getCategories`](../interfaces/RedTeamService.md#getcategories)

***

### getChannel()

> **getChannel**(`channelId`): `Promise`\<`RedTeamChannel`\>

Defined in: [src/airs/redteam.ts:604](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L604)

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

Defined in: [src/airs/redteam.ts:632](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L632)

Get network broker channel statistics.

#### Returns

`Promise`\<`RedTeamChannelStats`\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getChannelStats`](../interfaces/RedTeamService.md#getchannelstats)

***

### getCustomReport()

> **getCustomReport**(`jobId`): `Promise`\<[`RedTeamCustomReport`](../interfaces/RedTeamCustomReport.md)\>

Defined in: [src/airs/redteam.ts:488](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L488)

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

Defined in: [src/airs/redteam.ts:475](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L475)

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

Defined in: [src/airs/redteam.ts:166](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L166)

Get EULA content.

#### Returns

`Promise`\<`EulaContent`\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getEulaContent`](../interfaces/RedTeamService.md#geteulacontent)

***

### getEulaStatus()

> **getEulaStatus**(): `Promise`\<`EulaStatus`\>

Defined in: [src/airs/redteam.ts:171](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L171)

Get EULA acceptance status.

#### Returns

`Promise`\<`EulaStatus`\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getEulaStatus`](../interfaces/RedTeamService.md#geteulastatus)

***

### getInstance()

> **getInstance**(`tenantId`): `Promise`\<`InstanceDetail`\>

Defined in: [src/airs/redteam.ts:207](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L207)

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

Defined in: [src/airs/redteam.ts:645](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L645)

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

Defined in: [src/airs/redteam.ts:269](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L269)

Get or create registry credentials.

#### Returns

`Promise`\<`RegistryCredentials`\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getRegistryCredentials`](../interfaces/RedTeamService.md#getregistrycredentials)

***

### getScan()

> **getScan**(`jobId`): `Promise`\<[`RedTeamJob`](../interfaces/RedTeamJob.md)\>

Defined in: [src/airs/redteam.ts:415](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L415)

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

Defined in: [src/airs/redteam.ts:442](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L442)

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

Defined in: [src/airs/redteam.ts:332](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L332)

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

Defined in: [src/airs/redteam.ts:296](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L296)

Get target field metadata.

#### Returns

`Promise`\<`Record`\<`string`, `unknown`\>\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getTargetMetadata`](../interfaces/RedTeamService.md#gettargetmetadata)

***

### getTargetProfile()

> **getTargetProfile**(`uuid`): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: [src/airs/redteam.ts:368](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L368)

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

Defined in: [src/airs/redteam.ts:659](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L659)

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

Defined in: [src/airs/redteam.ts:300](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L300)

Get provider-specific target templates.

#### Returns

`Promise`\<`Record`\<`string`, `unknown`\>\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getTargetTemplates`](../interfaces/RedTeamService.md#gettargettemplates)

***

### listAttacks()

> **listAttacks**(`jobId`, `opts?`): `Promise`\<\{ `attacks`: [`RedTeamAttack`](../interfaces/RedTeamAttack.md)[]; `totalItems?`: `number`; \}\>

Defined in: [src/airs/redteam.ts:510](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L510)

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

Defined in: [src/airs/redteam.ts:587](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L587)

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

Defined in: [src/airs/redteam.ts:534](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L534)

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

Defined in: [src/airs/redteam.ts:420](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L420)

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

Defined in: [src/airs/redteam.ts:304](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L304)

List configured red team targets.

#### Returns

`Promise`\<[`RedTeamTarget`](../interfaces/RedTeamTarget.md)[]\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`listTargets`](../interfaces/RedTeamService.md#listtargets)

***

### probeTarget()

> **probeTarget**(`request`): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: [src/airs/redteam.ts:358](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L358)

Probe a target connection.

#### Parameters

##### request

`Record`\<`string`, `unknown`\>

#### Returns

`Promise`\<`Record`\<`string`, `unknown`\>\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`probeTarget`](../interfaces/RedTeamService.md#probetarget)

***

### updateChannel()

> **updateChannel**(`channelId`, `request`): `Promise`\<`RedTeamChannel`\>

Defined in: [src/airs/redteam.ts:618](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L618)

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

Defined in: [src/airs/redteam.ts:252](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L252)

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

Defined in: [src/airs/redteam.ts:217](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L217)

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

Defined in: [src/airs/redteam.ts:345](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L345)

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

Defined in: [src/airs/redteam.ts:373](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L373)

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

### validateTargetAuth()

> **validateTargetAuth**(`request`): `Promise`\<`TargetAuthValidationResult`\>

Defined in: [src/airs/redteam.ts:277](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L277)

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

Defined in: [src/airs/redteam.ts:565](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L565)

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
