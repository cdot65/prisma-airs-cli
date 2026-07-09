# Interface: RedTeamService

Defined in: [src/airs/types.ts:489](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L489)

Contract for AI Red Team scan operations.

## Methods

### abortScan()

> **abortScan**(`jobId`): `Promise`\<`void`\>

Defined in: [src/airs/types.ts:585](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L585)

Abort a running scan.

#### Parameters

##### jobId

`string`

#### Returns

`Promise`\<`void`\>

***

### acceptEula()

> **acceptEula**(`eulaContent`): `Promise`\<`EulaStatus`\>

Defined in: [src/airs/types.ts:495](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L495)

Accept the EULA.

#### Parameters

##### eulaContent

`string`

#### Returns

`Promise`\<`EulaStatus`\>

***

### createChannel()

> **createChannel**(`request`): `Promise`\<`RedTeamChannel`\>

Defined in: [src/airs/types.ts:622](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L622)

Create a network broker channel.

#### Parameters

##### request

`RedTeamChannelCreateRequest`

#### Returns

`Promise`\<`RedTeamChannel`\>

***

### createDevices()

> **createDevices**(`tenantId`, `request`): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: [src/airs/types.ts:506](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L506)

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

Defined in: [src/airs/types.ts:498](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L498)

Create an instance.

#### Parameters

##### request

`InstanceRequest`

#### Returns

`Promise`\<`InstanceResponse`\>

***

### createScan()

> **createScan**(`request`): `Promise`\<[`RedTeamJob`](RedTeamJob.md)\>

Defined in: [src/airs/types.ts:562](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L562)

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

Defined in: [src/airs/types.ts:534](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L534)

Create a red team target.

#### Parameters

##### request

[`RedTeamTargetCreateRequest`](RedTeamTargetCreateRequest.md)

##### opts?

[`TargetOperationOptions`](TargetOperationOptions.md)

#### Returns

`Promise`\<[`RedTeamTargetDetail`](RedTeamTargetDetail.md)\>

***

### deleteDevices()

> **deleteDevices**(`tenantId`, `serialNumbers`): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: [src/airs/types.ts:516](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L516)

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

Defined in: [src/airs/types.ts:504](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L504)

Delete an instance.

#### Parameters

##### tenantId

`string`

#### Returns

`Promise`\<`InstanceResponse`\>

***

### deleteTarget()

> **deleteTarget**(`uuid`): `Promise`\<`void`\>

Defined in: [src/airs/types.ts:547](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L547)

Delete a red team target.

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<`void`\>

***

### getCategories()

> **getCategories**(): `Promise`\<[`RedTeamCategory`](RedTeamCategory.md)[]\>

Defined in: [src/airs/types.ts:606](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L606)

List available attack categories.

#### Returns

`Promise`\<[`RedTeamCategory`](RedTeamCategory.md)[]\>

***

### getChannel()

> **getChannel**(`channelId`): `Promise`\<`RedTeamChannel`\>

Defined in: [src/airs/types.ts:620](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L620)

Get a network broker channel by ID.

#### Parameters

##### channelId

`string`

#### Returns

`Promise`\<`RedTeamChannel`\>

***

### getChannelStats()

> **getChannelStats**(): `Promise`\<`RedTeamChannelStats`\>

Defined in: [src/airs/types.ts:626](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L626)

Get network broker channel statistics.

#### Returns

`Promise`\<`RedTeamChannelStats`\>

***

### getCustomReport()

> **getCustomReport**(`jobId`): `Promise`\<[`RedTeamCustomReport`](RedTeamCustomReport.md)\>

Defined in: [src/airs/types.ts:594](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L594)

Get custom attack report.

#### Parameters

##### jobId

`string`

#### Returns

`Promise`\<[`RedTeamCustomReport`](RedTeamCustomReport.md)\>

***

### getDynamicReport()

> **getDynamicReport**(`jobId`): `Promise`\<`RedTeamDynamicReport`\>

Defined in: [src/airs/types.ts:591](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L591)

Get dynamic scan report.

#### Parameters

##### jobId

`string`

#### Returns

`Promise`\<`RedTeamDynamicReport`\>

***

### getEulaContent()

> **getEulaContent**(): `Promise`\<`EulaContent`\>

Defined in: [src/airs/types.ts:491](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L491)

Get EULA content.

#### Returns

`Promise`\<`EulaContent`\>

***

### getEulaStatus()

> **getEulaStatus**(): `Promise`\<`EulaStatus`\>

Defined in: [src/airs/types.ts:493](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L493)

Get EULA acceptance status.

#### Returns

`Promise`\<`EulaStatus`\>

***

### getInstance()

> **getInstance**(`tenantId`): `Promise`\<`InstanceDetail`\>

Defined in: [src/airs/types.ts:500](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L500)

Get instance details.

#### Parameters

##### tenantId

`string`

#### Returns

`Promise`\<`InstanceDetail`\>

***

### getLanguages()

> **getLanguages**(`management?`): `Promise`\<`RedTeamLanguages`\>

Defined in: [src/airs/types.ts:629](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L629)

List tenant languages (data plane, or management plane when `management`).

#### Parameters

##### management?

`boolean`

#### Returns

`Promise`\<`RedTeamLanguages`\>

***

### getRegistryCredentials()

> **getRegistryCredentials**(): `Promise`\<`RegistryCredentials`\>

Defined in: [src/airs/types.ts:518](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L518)

Get or create registry credentials.

#### Returns

`Promise`\<`RegistryCredentials`\>

***

### getScan()

> **getScan**(`jobId`): `Promise`\<[`RedTeamJob`](RedTeamJob.md)\>

Defined in: [src/airs/types.ts:574](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L574)

Get scan status by job ID.

#### Parameters

##### jobId

`string`

#### Returns

`Promise`\<[`RedTeamJob`](RedTeamJob.md)\>

***

### getStaticReport()

> **getStaticReport**(`jobId`): `Promise`\<[`RedTeamStaticReport`](RedTeamStaticReport.md)\>

Defined in: [src/airs/types.ts:588](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L588)

Get static scan report.

#### Parameters

##### jobId

`string`

#### Returns

`Promise`\<[`RedTeamStaticReport`](RedTeamStaticReport.md)\>

***

### getTarget()

> **getTarget**(`uuid`): `Promise`\<[`RedTeamTargetDetail`](RedTeamTargetDetail.md)\>

Defined in: [src/airs/types.ts:531](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L531)

Get target details.

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<[`RedTeamTargetDetail`](RedTeamTargetDetail.md)\>

***

### getTargetMetadata()

> **getTargetMetadata**(): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: [src/airs/types.ts:523](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L523)

Get target field metadata.

#### Returns

`Promise`\<`Record`\<`string`, `unknown`\>\>

***

### getTargetProfile()

> **getTargetProfile**(`uuid`): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: [src/airs/types.ts:553](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L553)

Get target profile.

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<`Record`\<`string`, `unknown`\>\>

***

### getTargetProfileErrorLogs()

> **getTargetProfileErrorLogs**(`targetId`, `opts?`): `Promise`\<\{ `logs`: `RedTeamErrorLog`[]; `totalItems?`: `number`; \}\>

Defined in: [src/airs/types.ts:631](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L631)

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

Defined in: [src/airs/types.ts:525](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L525)

Get provider-specific target templates.

#### Returns

`Promise`\<`Record`\<`string`, `unknown`\>\>

***

### listAttacks()

> **listAttacks**(`jobId`, `opts?`): `Promise`\<\{ `attacks`: [`RedTeamAttack`](RedTeamAttack.md)[]; `totalItems?`: `number`; \}\>

Defined in: [src/airs/types.ts:597](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L597)

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

Defined in: [src/airs/types.ts:616](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L616)

List network broker channels.

#### Parameters

##### opts?

`RedTeamChannelListOptions`

#### Returns

`Promise`\<\{ `channels`: `RedTeamChannel`[]; `totalItems?`: `number`; \}\>

***

### listCustomAttacks()

> **listCustomAttacks**(`jobId`, `opts?`): `Promise`\<[`RedTeamCustomAttack`](RedTeamCustomAttack.md)[]\>

Defined in: [src/airs/types.ts:603](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L603)

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

Defined in: [src/airs/types.ts:577](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L577)

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

Defined in: [src/airs/types.ts:528](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L528)

List configured red team targets.

#### Returns

`Promise`\<[`RedTeamTarget`](RedTeamTarget.md)[]\>

***

### probeTarget()

> **probeTarget**(`request`): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: [src/airs/types.ts:550](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L550)

Probe a target connection.

#### Parameters

##### request

`Record`\<`string`, `unknown`\>

#### Returns

`Promise`\<`Record`\<`string`, `unknown`\>\>

***

### updateChannel()

> **updateChannel**(`channelId`, `request`): `Promise`\<`RedTeamChannel`\>

Defined in: [src/airs/types.ts:624](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L624)

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

Defined in: [src/airs/types.ts:511](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L511)

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

Defined in: [src/airs/types.ts:502](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L502)

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

Defined in: [src/airs/types.ts:540](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L540)

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

Defined in: [src/airs/types.ts:556](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L556)

Update target profile.

#### Parameters

##### uuid

`string`

##### request

`Record`\<`string`, `unknown`\>

#### Returns

`Promise`\<`Record`\<`string`, `unknown`\>\>

***

### validateTargetAuth()

> **validateTargetAuth**(`request`): `Promise`\<`TargetAuthValidationResult`\>

Defined in: [src/airs/types.ts:521](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L521)

Validate target auth credentials.

#### Parameters

##### request

`TargetAuthValidationRequest`

#### Returns

`Promise`\<`TargetAuthValidationResult`\>

***

### waitForCompletion()

> **waitForCompletion**(`jobId`, `onProgress?`, `intervalMs?`): `Promise`\<[`RedTeamJob`](RedTeamJob.md)\>

Defined in: [src/airs/types.ts:609](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L609)

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
