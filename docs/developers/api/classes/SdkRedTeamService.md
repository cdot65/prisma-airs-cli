# Class: SdkRedTeamService

Defined in: [src/airs/redteam.ts:116](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L116)

Wraps the SDK's RedTeamClient to implement RedTeamService.
Provides scan creation, status polling, report retrieval, and target/category listing.

## Implements

- [`RedTeamService`](../interfaces/RedTeamService.md)

## Constructors

### Constructor

> **new SdkRedTeamService**(`opts?`): `SdkRedTeamService`

Defined in: [src/airs/redteam.ts:119](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L119)

#### Parameters

##### opts?

`RedTeamClientOptions`

#### Returns

`SdkRedTeamService`

## Methods

### abortScan()

> **abortScan**(`jobId`): `Promise`\<`void`\>

Defined in: [src/airs/redteam.ts:395](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L395)

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

Defined in: [src/airs/redteam.ts:137](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L137)

Accept the EULA.

#### Parameters

##### eulaContent

`string`

#### Returns

`Promise`\<`EulaStatus`\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`acceptEula`](../interfaces/RedTeamService.md#accepteula)

***

### createDevices()

> **createDevices**(`tenantId`, `request`): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: [src/airs/redteam.ts:199](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L199)

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

Defined in: [src/airs/redteam.ts:149](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L149)

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

Defined in: [src/airs/redteam.ts:338](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L338)

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

Defined in: [src/airs/redteam.ts:294](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L294)

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

Defined in: [src/airs/redteam.ts:219](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L219)

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

Defined in: [src/airs/redteam.ts:189](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L189)

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

Defined in: [src/airs/redteam.ts:311](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L311)

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

Defined in: [src/airs/redteam.ts:495](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L495)

List available attack categories.

#### Returns

`Promise`\<[`RedTeamCategory`](../interfaces/RedTeamCategory.md)[]\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getCategories`](../interfaces/RedTeamService.md#getcategories)

***

### getCustomReport()

> **getCustomReport**(`jobId`): `Promise`\<[`RedTeamCustomReport`](../interfaces/RedTeamCustomReport.md)\>

Defined in: [src/airs/redteam.ts:432](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L432)

Get custom attack report.

#### Parameters

##### jobId

`string`

#### Returns

`Promise`\<[`RedTeamCustomReport`](../interfaces/RedTeamCustomReport.md)\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getCustomReport`](../interfaces/RedTeamService.md#getcustomreport)

***

### getEulaContent()

> **getEulaContent**(): `Promise`\<`EulaContent`\>

Defined in: [src/airs/redteam.ts:123](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L123)

Get EULA content.

#### Returns

`Promise`\<`EulaContent`\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getEulaContent`](../interfaces/RedTeamService.md#geteulacontent)

***

### getEulaStatus()

> **getEulaStatus**(): `Promise`\<`EulaStatus`\>

Defined in: [src/airs/redteam.ts:128](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L128)

Get EULA acceptance status.

#### Returns

`Promise`\<`EulaStatus`\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getEulaStatus`](../interfaces/RedTeamService.md#geteulastatus)

***

### getInstance()

> **getInstance**(`tenantId`): `Promise`\<`InstanceDetail`\>

Defined in: [src/airs/redteam.ts:164](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L164)

Get instance details.

#### Parameters

##### tenantId

`string`

#### Returns

`Promise`\<`InstanceDetail`\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getInstance`](../interfaces/RedTeamService.md#getinstance)

***

### getRegistryCredentials()

> **getRegistryCredentials**(): `Promise`\<`RegistryCredentials`\>

Defined in: [src/airs/redteam.ts:226](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L226)

Get or create registry credentials.

#### Returns

`Promise`\<`RegistryCredentials`\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getRegistryCredentials`](../interfaces/RedTeamService.md#getregistrycredentials)

***

### getScan()

> **getScan**(`jobId`): `Promise`\<[`RedTeamJob`](../interfaces/RedTeamJob.md)\>

Defined in: [src/airs/redteam.ts:372](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L372)

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

Defined in: [src/airs/redteam.ts:399](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L399)

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

Defined in: [src/airs/redteam.ts:289](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L289)

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

Defined in: [src/airs/redteam.ts:253](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L253)

Get target field metadata.

#### Returns

`Promise`\<`Record`\<`string`, `unknown`\>\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getTargetMetadata`](../interfaces/RedTeamService.md#gettargetmetadata)

***

### getTargetProfile()

> **getTargetProfile**(`uuid`): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: [src/airs/redteam.ts:325](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L325)

Get target profile.

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<`Record`\<`string`, `unknown`\>\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getTargetProfile`](../interfaces/RedTeamService.md#gettargetprofile)

***

### getTargetTemplates()

> **getTargetTemplates**(): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: [src/airs/redteam.ts:257](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L257)

Get provider-specific target templates.

#### Returns

`Promise`\<`Record`\<`string`, `unknown`\>\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getTargetTemplates`](../interfaces/RedTeamService.md#gettargettemplates)

***

### listAttacks()

> **listAttacks**(`jobId`, `opts?`): `Promise`\<\{ `attacks`: [`RedTeamAttack`](../interfaces/RedTeamAttack.md)[]; `totalItems?`: `number`; \}\>

Defined in: [src/airs/redteam.ts:454](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L454)

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

### listCustomAttacks()

> **listCustomAttacks**(`jobId`, `opts?`): `Promise`\<[`RedTeamCustomAttack`](../interfaces/RedTeamCustomAttack.md)[]\>

Defined in: [src/airs/redteam.ts:478](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L478)

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

Defined in: [src/airs/redteam.ts:377](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L377)

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

Defined in: [src/airs/redteam.ts:261](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L261)

List configured red team targets.

#### Returns

`Promise`\<[`RedTeamTarget`](../interfaces/RedTeamTarget.md)[]\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`listTargets`](../interfaces/RedTeamService.md#listtargets)

***

### probeTarget()

> **probeTarget**(`request`): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: [src/airs/redteam.ts:315](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L315)

Probe a target connection.

#### Parameters

##### request

`Record`\<`string`, `unknown`\>

#### Returns

`Promise`\<`Record`\<`string`, `unknown`\>\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`probeTarget`](../interfaces/RedTeamService.md#probetarget)

***

### updateDevices()

> **updateDevices**(`tenantId`, `request`): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: [src/airs/redteam.ts:209](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L209)

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

Defined in: [src/airs/redteam.ts:174](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L174)

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

Defined in: [src/airs/redteam.ts:302](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L302)

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

Defined in: [src/airs/redteam.ts:330](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L330)

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

Defined in: [src/airs/redteam.ts:234](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L234)

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

Defined in: [src/airs/redteam.ts:509](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L509)

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
