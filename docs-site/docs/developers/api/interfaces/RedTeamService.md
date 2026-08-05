# Interface: RedTeamService

Defined in: [src/airs/types.ts:548](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L548)

Contract for AI Red Team scan operations.

## Methods

### abortScan()

> **abortScan**(`jobId`): `Promise`\<`void`\>

Defined in: [src/airs/types.ts:644](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L644)

Abort a running scan.

#### Parameters

##### jobId

`string`

#### Returns

`Promise`\<`void`\>

***

### acceptEula()

> **acceptEula**(`eulaContent`): `Promise`\<`EulaStatus`\>

Defined in: [src/airs/types.ts:554](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L554)

Accept the EULA.

#### Parameters

##### eulaContent

`string`

#### Returns

`Promise`\<`EulaStatus`\>

***

### createAdapter()

> **createAdapter**(`request`, `validate?`): `Promise`\<`RedTeamAdapterDetail`\>

Defined in: [src/airs/types.ts:700](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L700)

#### Parameters

##### request

`RedTeamAdapterCreateRequest`

##### validate?

`boolean`

#### Returns

`Promise`\<`RedTeamAdapterDetail`\>

***

### createChannel()

> **createChannel**(`request`): `Promise`\<`RedTeamChannel`\>

Defined in: [src/airs/types.ts:681](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L681)

Create a network broker channel.

#### Parameters

##### request

`RedTeamChannelCreateRequest`

#### Returns

`Promise`\<`RedTeamChannel`\>

***

### createDevices()

> **createDevices**(`tenantId`, `request`): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: [src/airs/types.ts:565](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L565)

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

> **createInstance**(`request`): `Promise`\<`InstanceResponse`\>

Defined in: [src/airs/types.ts:557](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L557)

Create an instance.

#### Parameters

##### request

`InstanceRequest`

#### Returns

`Promise`\<`InstanceResponse`\>

***

### createScan()

> **createScan**(`request`): `Promise`\<[`RedTeamJob`](RedTeamJob.md)\>

Defined in: [src/airs/types.ts:621](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L621)

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

Defined in: [src/airs/types.ts:593](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L593)

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

Defined in: [src/airs/types.ts:710](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L710)

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<`void`\>

***

### deleteDevices()

> **deleteDevices**(`tenantId`, `serialNumbers`): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: [src/airs/types.ts:575](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L575)

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

> **deleteInstance**(`tenantId`): `Promise`\<`InstanceResponse`\>

Defined in: [src/airs/types.ts:563](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L563)

Delete an instance.

#### Parameters

##### tenantId

`string`

#### Returns

`Promise`\<`InstanceResponse`\>

***

### deleteTarget()

> **deleteTarget**(`uuid`): `Promise`\<`void`\>

Defined in: [src/airs/types.ts:606](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L606)

Delete a red team target.

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<`void`\>

***

### getAdapter()

> **getAdapter**(`uuid`): `Promise`\<`RedTeamAdapterDetail`\>

Defined in: [src/airs/types.ts:699](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L699)

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<`RedTeamAdapterDetail`\>

***

### getCategories()

> **getCategories**(): `Promise`\<[`RedTeamCategory`](RedTeamCategory.md)[]\>

Defined in: [src/airs/types.ts:665](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L665)

List available attack categories.

#### Returns

`Promise`\<[`RedTeamCategory`](RedTeamCategory.md)[]\>

***

### getChannel()

> **getChannel**(`channelId`): `Promise`\<`RedTeamChannel`\>

Defined in: [src/airs/types.ts:679](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L679)

Get a network broker channel by ID.

#### Parameters

##### channelId

`string`

#### Returns

`Promise`\<`RedTeamChannel`\>

***

### getChannelStats()

> **getChannelStats**(): `Promise`\<`RedTeamChannelStats`\>

Defined in: [src/airs/types.ts:685](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L685)

Get network broker channel statistics.

#### Returns

`Promise`\<`RedTeamChannelStats`\>

***

### getCustomReport()

> **getCustomReport**(`jobId`): `Promise`\<[`RedTeamCustomReport`](RedTeamCustomReport.md)\>

Defined in: [src/airs/types.ts:653](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L653)

Get custom attack report.

#### Parameters

##### jobId

`string`

#### Returns

`Promise`\<[`RedTeamCustomReport`](RedTeamCustomReport.md)\>

***

### getDynamicReport()

> **getDynamicReport**(`jobId`): `Promise`\<`RedTeamDynamicReport`\>

Defined in: [src/airs/types.ts:650](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L650)

Get dynamic scan report.

#### Parameters

##### jobId

`string`

#### Returns

`Promise`\<`RedTeamDynamicReport`\>

***

### getEulaContent()

> **getEulaContent**(): `Promise`\<`EulaContent`\>

Defined in: [src/airs/types.ts:550](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L550)

Get EULA content.

#### Returns

`Promise`\<`EulaContent`\>

***

### getEulaStatus()

> **getEulaStatus**(): `Promise`\<`EulaStatus`\>

Defined in: [src/airs/types.ts:552](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L552)

Get EULA acceptance status.

#### Returns

`Promise`\<`EulaStatus`\>

***

### getInstance()

> **getInstance**(`tenantId`): `Promise`\<`InstanceDetail`\>

Defined in: [src/airs/types.ts:559](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L559)

Get instance details.

#### Parameters

##### tenantId

`string`

#### Returns

`Promise`\<`InstanceDetail`\>

***

### getLanguages()

> **getLanguages**(`management?`): `Promise`\<`RedTeamLanguages`\>

Defined in: [src/airs/types.ts:688](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L688)

List tenant languages (data plane, or management plane when `management`).

#### Parameters

##### management?

`boolean`

#### Returns

`Promise`\<`RedTeamLanguages`\>

***

### getRegistryCredentials()

> **getRegistryCredentials**(): `Promise`\<`RegistryCredentials`\>

Defined in: [src/airs/types.ts:577](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L577)

Get or create registry credentials.

#### Returns

`Promise`\<`RegistryCredentials`\>

***

### getScan()

> **getScan**(`jobId`): `Promise`\<[`RedTeamJob`](RedTeamJob.md)\>

Defined in: [src/airs/types.ts:633](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L633)

Get scan status by job ID.

#### Parameters

##### jobId

`string`

#### Returns

`Promise`\<[`RedTeamJob`](RedTeamJob.md)\>

***

### getStaticReport()

> **getStaticReport**(`jobId`): `Promise`\<[`RedTeamStaticReport`](RedTeamStaticReport.md)\>

Defined in: [src/airs/types.ts:647](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L647)

Get static scan report.

#### Parameters

##### jobId

`string`

#### Returns

`Promise`\<[`RedTeamStaticReport`](RedTeamStaticReport.md)\>

***

### getTarget()

> **getTarget**(`uuid`): `Promise`\<[`RedTeamTargetDetail`](RedTeamTargetDetail.md)\>

Defined in: [src/airs/types.ts:590](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L590)

Get target details.

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<[`RedTeamTargetDetail`](RedTeamTargetDetail.md)\>

***

### getTargetMetadata()

> **getTargetMetadata**(): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: [src/airs/types.ts:582](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L582)

Get target field metadata.

#### Returns

`Promise`\<`Record`\<`string`, `unknown`\>\>

***

### getTargetProfile()

> **getTargetProfile**(`uuid`): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: [src/airs/types.ts:612](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L612)

Get target profile.

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<`Record`\<`string`, `unknown`\>\>

***

### getTargetProfileErrorLogs()

> **getTargetProfileErrorLogs**(`targetId`, `opts?`): `Promise`\<\{ `logs`: `RedTeamErrorLog`[]; `totalItems?`: `number`; \}\>

Defined in: [src/airs/types.ts:690](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L690)

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

***

### getTargetTemplates()

> **getTargetTemplates**(): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: [src/airs/types.ts:584](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L584)

Get provider-specific target templates.

#### Returns

`Promise`\<`Record`\<`string`, `unknown`\>\>

***

### listAdapters()

> **listAdapters**(`opts?`): `Promise`\<\{ `adapters`: `RedTeamAdapterListItem`[]; `totalItems?`: `number`; \}\>

Defined in: [src/airs/types.ts:696](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L696)

#### Parameters

##### opts?

`RedTeamAdapterListOptions`

#### Returns

`Promise`\<\{ `adapters`: `RedTeamAdapterListItem`[]; `totalItems?`: `number`; \}\>

***

### listAttacks()

> **listAttacks**(`jobId`, `opts?`): `Promise`\<\{ `attacks`: [`RedTeamAttack`](RedTeamAttack.md)[]; `totalItems?`: `number`; \}\>

Defined in: [src/airs/types.ts:656](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L656)

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

> **listChannels**(`opts?`): `Promise`\<\{ `channels`: `RedTeamChannel`[]; `totalItems?`: `number`; \}\>

Defined in: [src/airs/types.ts:675](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L675)

List network broker channels.

#### Parameters

##### opts?

`RedTeamChannelListOptions`

#### Returns

`Promise`\<\{ `channels`: `RedTeamChannel`[]; `totalItems?`: `number`; \}\>

***

### listCustomAttacks()

> **listCustomAttacks**(`jobId`, `opts?`): `Promise`\<[`RedTeamCustomAttack`](RedTeamCustomAttack.md)[]\>

Defined in: [src/airs/types.ts:662](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L662)

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

Defined in: [src/airs/types.ts:636](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L636)

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

`Promise`\<[`RedTeamJob`](RedTeamJob.md)[]\>

***

### listTargets()

> **listTargets**(): `Promise`\<[`RedTeamTarget`](RedTeamTarget.md)[]\>

Defined in: [src/airs/types.ts:587](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L587)

List configured red team targets.

#### Returns

`Promise`\<[`RedTeamTarget`](RedTeamTarget.md)[]\>

***

### probeTarget()

> **probeTarget**(`request`): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: [src/airs/types.ts:609](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L609)

Probe a target connection.

#### Parameters

##### request

`Record`\<`string`, `unknown`\>

#### Returns

`Promise`\<`Record`\<`string`, `unknown`\>\>

***

### updateAdapter()

> **updateAdapter**(`uuid`, `overrides`, `validate?`): `Promise`\<`RedTeamAdapterDetail`\>

Defined in: [src/airs/types.ts:705](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L705)

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

***

### updateChannel()

> **updateChannel**(`channelId`, `request`): `Promise`\<`RedTeamChannel`\>

Defined in: [src/airs/types.ts:683](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L683)

Update a network broker channel.

#### Parameters

##### channelId

`string`

##### request

`RedTeamChannelUpdateRequest`

#### Returns

`Promise`\<`RedTeamChannel`\>

***

### updateDevices()

> **updateDevices**(`tenantId`, `request`): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: [src/airs/types.ts:570](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L570)

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

> **updateInstance**(`tenantId`, `request`): `Promise`\<`InstanceResponse`\>

Defined in: [src/airs/types.ts:561](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L561)

Update an instance.

#### Parameters

##### tenantId

`string`

##### request

`InstanceRequest`

#### Returns

`Promise`\<`InstanceResponse`\>

***

### updateTarget()

> **updateTarget**(`uuid`, `request`, `opts?`): `Promise`\<[`RedTeamTargetDetail`](RedTeamTargetDetail.md)\>

Defined in: [src/airs/types.ts:599](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L599)

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

Defined in: [src/airs/types.ts:615](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L615)

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

> **validateAdapter**(`request`): `Promise`\<`RedTeamAdapterValidationResult`\>

Defined in: [src/airs/types.ts:712](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L712)

Run a script end-to-end through the broker channel; returns an execution outcome.

#### Parameters

##### request

`RedTeamAdapterValidateRequest`

#### Returns

`Promise`\<`RedTeamAdapterValidationResult`\>

***

### validateTargetAuth()

> **validateTargetAuth**(`request`): `Promise`\<`TargetAuthValidationResult`\>

Defined in: [src/airs/types.ts:580](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L580)

Validate target auth credentials.

#### Parameters

##### request

`TargetAuthValidationRequest`

#### Returns

`Promise`\<`TargetAuthValidationResult`\>

***

### waitForCompletion()

> **waitForCompletion**(`jobId`, `onProgress?`, `intervalMs?`): `Promise`\<[`RedTeamJob`](RedTeamJob.md)\>

Defined in: [src/airs/types.ts:668](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L668)

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
