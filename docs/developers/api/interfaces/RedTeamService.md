# Interface: RedTeamService

Defined in: [src/airs/types.ts:404](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L404)

Contract for AI Red Team scan operations.

## Methods

### abortScan()

> **abortScan**(`jobId`): `Promise`\<`void`\>

Defined in: [src/airs/types.ts:500](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L500)

Abort a running scan.

#### Parameters

##### jobId

`string`

#### Returns

`Promise`\<`void`\>

***

### acceptEula()

> **acceptEula**(`eulaContent`): `Promise`\<`EulaStatus`\>

Defined in: [src/airs/types.ts:410](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L410)

Accept the EULA.

#### Parameters

##### eulaContent

`string`

#### Returns

`Promise`\<`EulaStatus`\>

***

### createDevices()

> **createDevices**(`tenantId`, `request`): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: [src/airs/types.ts:421](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L421)

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

Defined in: [src/airs/types.ts:413](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L413)

Create an instance.

#### Parameters

##### request

`InstanceRequest`

#### Returns

`Promise`\<`InstanceResponse`\>

***

### createScan()

> **createScan**(`request`): `Promise`\<[`RedTeamJob`](RedTeamJob.md)\>

Defined in: [src/airs/types.ts:477](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L477)

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

Defined in: [src/airs/types.ts:449](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L449)

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

Defined in: [src/airs/types.ts:431](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L431)

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

Defined in: [src/airs/types.ts:419](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L419)

Delete an instance.

#### Parameters

##### tenantId

`string`

#### Returns

`Promise`\<`InstanceResponse`\>

***

### deleteTarget()

> **deleteTarget**(`uuid`): `Promise`\<`void`\>

Defined in: [src/airs/types.ts:462](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L462)

Delete a red team target.

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<`void`\>

***

### getCategories()

> **getCategories**(): `Promise`\<[`RedTeamCategory`](RedTeamCategory.md)[]\>

Defined in: [src/airs/types.ts:521](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L521)

List available attack categories.

#### Returns

`Promise`\<[`RedTeamCategory`](RedTeamCategory.md)[]\>

***

### getCustomReport()

> **getCustomReport**(`jobId`): `Promise`\<[`RedTeamCustomReport`](RedTeamCustomReport.md)\>

Defined in: [src/airs/types.ts:509](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L509)

Get custom attack report.

#### Parameters

##### jobId

`string`

#### Returns

`Promise`\<[`RedTeamCustomReport`](RedTeamCustomReport.md)\>

***

### getDynamicReport()

> **getDynamicReport**(`jobId`): `Promise`\<`RedTeamDynamicReport`\>

Defined in: [src/airs/types.ts:506](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L506)

Get dynamic scan report.

#### Parameters

##### jobId

`string`

#### Returns

`Promise`\<`RedTeamDynamicReport`\>

***

### getEulaContent()

> **getEulaContent**(): `Promise`\<`EulaContent`\>

Defined in: [src/airs/types.ts:406](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L406)

Get EULA content.

#### Returns

`Promise`\<`EulaContent`\>

***

### getEulaStatus()

> **getEulaStatus**(): `Promise`\<`EulaStatus`\>

Defined in: [src/airs/types.ts:408](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L408)

Get EULA acceptance status.

#### Returns

`Promise`\<`EulaStatus`\>

***

### getInstance()

> **getInstance**(`tenantId`): `Promise`\<`InstanceDetail`\>

Defined in: [src/airs/types.ts:415](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L415)

Get instance details.

#### Parameters

##### tenantId

`string`

#### Returns

`Promise`\<`InstanceDetail`\>

***

### getRegistryCredentials()

> **getRegistryCredentials**(): `Promise`\<`RegistryCredentials`\>

Defined in: [src/airs/types.ts:433](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L433)

Get or create registry credentials.

#### Returns

`Promise`\<`RegistryCredentials`\>

***

### getScan()

> **getScan**(`jobId`): `Promise`\<[`RedTeamJob`](RedTeamJob.md)\>

Defined in: [src/airs/types.ts:489](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L489)

Get scan status by job ID.

#### Parameters

##### jobId

`string`

#### Returns

`Promise`\<[`RedTeamJob`](RedTeamJob.md)\>

***

### getStaticReport()

> **getStaticReport**(`jobId`): `Promise`\<[`RedTeamStaticReport`](RedTeamStaticReport.md)\>

Defined in: [src/airs/types.ts:503](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L503)

Get static scan report.

#### Parameters

##### jobId

`string`

#### Returns

`Promise`\<[`RedTeamStaticReport`](RedTeamStaticReport.md)\>

***

### getTarget()

> **getTarget**(`uuid`): `Promise`\<[`RedTeamTargetDetail`](RedTeamTargetDetail.md)\>

Defined in: [src/airs/types.ts:446](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L446)

Get target details.

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<[`RedTeamTargetDetail`](RedTeamTargetDetail.md)\>

***

### getTargetMetadata()

> **getTargetMetadata**(): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: [src/airs/types.ts:438](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L438)

Get target field metadata.

#### Returns

`Promise`\<`Record`\<`string`, `unknown`\>\>

***

### getTargetProfile()

> **getTargetProfile**(`uuid`): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: [src/airs/types.ts:468](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L468)

Get target profile.

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<`Record`\<`string`, `unknown`\>\>

***

### getTargetTemplates()

> **getTargetTemplates**(): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: [src/airs/types.ts:440](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L440)

Get provider-specific target templates.

#### Returns

`Promise`\<`Record`\<`string`, `unknown`\>\>

***

### listAttacks()

> **listAttacks**(`jobId`, `opts?`): `Promise`\<\{ `attacks`: [`RedTeamAttack`](RedTeamAttack.md)[]; `totalItems?`: `number`; \}\>

Defined in: [src/airs/types.ts:512](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L512)

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

### listCustomAttacks()

> **listCustomAttacks**(`jobId`, `opts?`): `Promise`\<[`RedTeamCustomAttack`](RedTeamCustomAttack.md)[]\>

Defined in: [src/airs/types.ts:518](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L518)

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

Defined in: [src/airs/types.ts:492](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L492)

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

Defined in: [src/airs/types.ts:443](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L443)

List configured red team targets.

#### Returns

`Promise`\<[`RedTeamTarget`](RedTeamTarget.md)[]\>

***

### probeTarget()

> **probeTarget**(`request`): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: [src/airs/types.ts:465](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L465)

Probe a target connection.

#### Parameters

##### request

`Record`\<`string`, `unknown`\>

#### Returns

`Promise`\<`Record`\<`string`, `unknown`\>\>

***

### updateDevices()

> **updateDevices**(`tenantId`, `request`): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: [src/airs/types.ts:426](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L426)

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

Defined in: [src/airs/types.ts:417](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L417)

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

Defined in: [src/airs/types.ts:455](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L455)

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

Defined in: [src/airs/types.ts:471](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L471)

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

Defined in: [src/airs/types.ts:436](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L436)

Validate target auth credentials.

#### Parameters

##### request

`TargetAuthValidationRequest`

#### Returns

`Promise`\<`TargetAuthValidationResult`\>

***

### waitForCompletion()

> **waitForCompletion**(`jobId`, `onProgress?`, `intervalMs?`): `Promise`\<[`RedTeamJob`](RedTeamJob.md)\>

Defined in: [src/airs/types.ts:524](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L524)

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
