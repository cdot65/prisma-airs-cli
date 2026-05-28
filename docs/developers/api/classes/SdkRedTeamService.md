# Class: SdkRedTeamService

Defined in: [src/airs/redteam.ts:117](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L117)

Wraps the SDK's RedTeamClient to implement RedTeamService.
Provides scan creation, status polling, report retrieval, and target/category listing.

## Implements

- [`RedTeamService`](../interfaces/RedTeamService.md)

## Constructors

### Constructor

> **new SdkRedTeamService**(`opts?`): `SdkRedTeamService`

Defined in: [src/airs/redteam.ts:120](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L120)

#### Parameters

##### opts?

`RedTeamClientOptions`

#### Returns

`SdkRedTeamService`

## Methods

### abortScan()

> **abortScan**(`jobId`): `Promise`\<`void`\>

Defined in: [src/airs/redteam.ts:396](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L396)

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

Defined in: [src/airs/redteam.ts:138](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L138)

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

Defined in: [src/airs/redteam.ts:200](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L200)

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

Defined in: [src/airs/redteam.ts:150](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L150)

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

Defined in: [src/airs/redteam.ts:339](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L339)

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

Defined in: [src/airs/redteam.ts:295](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L295)

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

Defined in: [src/airs/redteam.ts:220](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L220)

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

Defined in: [src/airs/redteam.ts:190](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L190)

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

Defined in: [src/airs/redteam.ts:312](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L312)

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

Defined in: [src/airs/redteam.ts:509](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L509)

List available attack categories.

#### Returns

`Promise`\<[`RedTeamCategory`](../interfaces/RedTeamCategory.md)[]\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getCategories`](../interfaces/RedTeamService.md#getcategories)

***

### getCustomReport()

> **getCustomReport**(`jobId`): `Promise`\<[`RedTeamCustomReport`](../interfaces/RedTeamCustomReport.md)\>

Defined in: [src/airs/redteam.ts:446](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L446)

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

Defined in: [src/airs/redteam.ts:433](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L433)

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

Defined in: [src/airs/redteam.ts:124](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L124)

Get EULA content.

#### Returns

`Promise`\<`EulaContent`\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getEulaContent`](../interfaces/RedTeamService.md#geteulacontent)

***

### getEulaStatus()

> **getEulaStatus**(): `Promise`\<`EulaStatus`\>

Defined in: [src/airs/redteam.ts:129](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L129)

Get EULA acceptance status.

#### Returns

`Promise`\<`EulaStatus`\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getEulaStatus`](../interfaces/RedTeamService.md#geteulastatus)

***

### getInstance()

> **getInstance**(`tenantId`): `Promise`\<`InstanceDetail`\>

Defined in: [src/airs/redteam.ts:165](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L165)

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

Defined in: [src/airs/redteam.ts:227](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L227)

Get or create registry credentials.

#### Returns

`Promise`\<`RegistryCredentials`\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getRegistryCredentials`](../interfaces/RedTeamService.md#getregistrycredentials)

***

### getScan()

> **getScan**(`jobId`): `Promise`\<[`RedTeamJob`](../interfaces/RedTeamJob.md)\>

Defined in: [src/airs/redteam.ts:373](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L373)

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

Defined in: [src/airs/redteam.ts:400](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L400)

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

Defined in: [src/airs/redteam.ts:290](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L290)

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

Defined in: [src/airs/redteam.ts:254](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L254)

Get target field metadata.

#### Returns

`Promise`\<`Record`\<`string`, `unknown`\>\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getTargetMetadata`](../interfaces/RedTeamService.md#gettargetmetadata)

***

### getTargetProfile()

> **getTargetProfile**(`uuid`): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: [src/airs/redteam.ts:326](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L326)

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

Defined in: [src/airs/redteam.ts:258](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L258)

Get provider-specific target templates.

#### Returns

`Promise`\<`Record`\<`string`, `unknown`\>\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`getTargetTemplates`](../interfaces/RedTeamService.md#gettargettemplates)

***

### listAttacks()

> **listAttacks**(`jobId`, `opts?`): `Promise`\<\{ `attacks`: [`RedTeamAttack`](../interfaces/RedTeamAttack.md)[]; `totalItems?`: `number`; \}\>

Defined in: [src/airs/redteam.ts:468](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L468)

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

Defined in: [src/airs/redteam.ts:492](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L492)

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

Defined in: [src/airs/redteam.ts:378](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L378)

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

Defined in: [src/airs/redteam.ts:262](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L262)

List configured red team targets.

#### Returns

`Promise`\<[`RedTeamTarget`](../interfaces/RedTeamTarget.md)[]\>

#### Implementation of

[`RedTeamService`](../interfaces/RedTeamService.md).[`listTargets`](../interfaces/RedTeamService.md#listtargets)

***

### probeTarget()

> **probeTarget**(`request`): `Promise`\<`Record`\<`string`, `unknown`\>\>

Defined in: [src/airs/redteam.ts:316](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L316)

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

Defined in: [src/airs/redteam.ts:210](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L210)

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

Defined in: [src/airs/redteam.ts:175](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L175)

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

Defined in: [src/airs/redteam.ts:303](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L303)

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

Defined in: [src/airs/redteam.ts:331](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L331)

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

Defined in: [src/airs/redteam.ts:235](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L235)

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

Defined in: [src/airs/redteam.ts:523](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/redteam.ts#L523)

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
