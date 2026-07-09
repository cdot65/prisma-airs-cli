# Class: SdkModelSecurityService

Defined in: [src/airs/modelsecurity.ts:198](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/modelsecurity.ts#L198)

Wraps the SDK's ModelSecurityClient to implement ModelSecurityService.
Provides security group CRUD, rule browsing, scan operations, and label management.

## Implements

- [`ModelSecurityService`](../interfaces/ModelSecurityService.md)

## Constructors

### Constructor

> **new SdkModelSecurityService**(`opts?`): `SdkModelSecurityService`

Defined in: [src/airs/modelsecurity.ts:201](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/modelsecurity.ts#L201)

#### Parameters

##### opts?

`ModelSecurityClientOptions`

#### Returns

`SdkModelSecurityService`

## Methods

### addLabels()

> **addLabels**(`scanUuid`, `labels`): `Promise`\<`void`\>

Defined in: [src/airs/modelsecurity.ts:470](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/modelsecurity.ts#L470)

#### Parameters

##### scanUuid

`string`

##### labels

[`ModelSecurityLabel`](../interfaces/ModelSecurityLabel.md)[]

#### Returns

`Promise`\<`void`\>

#### Implementation of

[`ModelSecurityService`](../interfaces/ModelSecurityService.md).[`addLabels`](../interfaces/ModelSecurityService.md#addlabels)

***

### createGroup()

> **createGroup**(`request`): `Promise`\<[`ModelSecurityGroup`](../interfaces/ModelSecurityGroup.md)\>

Defined in: [src/airs/modelsecurity.ts:237](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/modelsecurity.ts#L237)

#### Parameters

##### request

[`ModelSecurityGroupCreateRequest`](../interfaces/ModelSecurityGroupCreateRequest.md)

#### Returns

`Promise`\<[`ModelSecurityGroup`](../interfaces/ModelSecurityGroup.md)\>

#### Implementation of

[`ModelSecurityService`](../interfaces/ModelSecurityService.md).[`createGroup`](../interfaces/ModelSecurityService.md#creategroup)

***

### createScan()

> **createScan**(`request`): `Promise`\<[`ModelSecurityScan`](../interfaces/ModelSecurityScan.md)\>

Defined in: [src/airs/modelsecurity.ts:367](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/modelsecurity.ts#L367)

#### Parameters

##### request

`Record`\<`string`, `unknown`\>

#### Returns

`Promise`\<[`ModelSecurityScan`](../interfaces/ModelSecurityScan.md)\>

#### Implementation of

[`ModelSecurityService`](../interfaces/ModelSecurityService.md).[`createScan`](../interfaces/ModelSecurityService.md#createscan)

***

### deleteGroup()

> **deleteGroup**(`uuid`): `Promise`\<`void`\>

Defined in: [src/airs/modelsecurity.ts:257](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/modelsecurity.ts#L257)

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<`void`\>

#### Implementation of

[`ModelSecurityService`](../interfaces/ModelSecurityService.md).[`deleteGroup`](../interfaces/ModelSecurityService.md#deletegroup)

***

### deleteGroupAndVerify()

> **deleteGroupAndVerify**(`uuid`): `Promise`\<\{ `confirmed`: `boolean`; `state?`: `string`; \}\>

Defined in: [src/airs/modelsecurity.ts:269](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/modelsecurity.ts#L269)

Delete a group, then verify whether it is actually gone. The API soft/async-deletes
security groups — a successful DELETE does not immediately remove the resource, so a
follow-up `get` may still return it as ACTIVE (see prisma-airs-cli#239). The SDK delete
returns void, so re-reading is the only way to report the real outcome rather than
claiming an unconditional success. Returns `{ confirmed: true }` when the group no longer
resolves, or `{ confirmed: false, state }` when it still does.

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<\{ `confirmed`: `boolean`; `state?`: `string`; \}\>

***

### deleteLabels()

> **deleteLabels**(`scanUuid`, `keys`): `Promise`\<`void`\>

Defined in: [src/airs/modelsecurity.ts:478](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/modelsecurity.ts#L478)

#### Parameters

##### scanUuid

`string`

##### keys

`string`[]

#### Returns

`Promise`\<`void`\>

#### Implementation of

[`ModelSecurityService`](../interfaces/ModelSecurityService.md).[`deleteLabels`](../interfaces/ModelSecurityService.md#deletelabels)

***

### getEvaluation()

> **getEvaluation**(`uuid`): `Promise`\<[`ModelSecurityEvaluation`](../interfaces/ModelSecurityEvaluation.md)\>

Defined in: [src/airs/modelsecurity.ts:418](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/modelsecurity.ts#L418)

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<[`ModelSecurityEvaluation`](../interfaces/ModelSecurityEvaluation.md)\>

#### Implementation of

[`ModelSecurityService`](../interfaces/ModelSecurityService.md).[`getEvaluation`](../interfaces/ModelSecurityService.md#getevaluation)

***

### getEvaluations()

> **getEvaluations**(`scanUuid`, `opts?`): `Promise`\<\{ `evaluations`: [`ModelSecurityEvaluation`](../interfaces/ModelSecurityEvaluation.md)[]; `totalItems`: `number`; \}\>

Defined in: [src/airs/modelsecurity.ts:403](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/modelsecurity.ts#L403)

#### Parameters

##### scanUuid

`string`

##### opts?

###### limit?

`number`

###### skip?

`number`

#### Returns

`Promise`\<\{ `evaluations`: [`ModelSecurityEvaluation`](../interfaces/ModelSecurityEvaluation.md)[]; `totalItems`: `number`; \}\>

#### Implementation of

[`ModelSecurityService`](../interfaces/ModelSecurityService.md).[`getEvaluations`](../interfaces/ModelSecurityService.md#getevaluations)

***

### getFiles()

> **getFiles**(`scanUuid`, `opts?`): `Promise`\<\{ `files`: [`ModelSecurityFile`](../interfaces/ModelSecurityFile.md)[]; `totalItems`: `number`; \}\>

Defined in: [src/airs/modelsecurity.ts:451](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/modelsecurity.ts#L451)

#### Parameters

##### scanUuid

`string`

##### opts?

[`ModelSecurityFileListOptions`](../interfaces/ModelSecurityFileListOptions.md)

#### Returns

`Promise`\<\{ `files`: [`ModelSecurityFile`](../interfaces/ModelSecurityFile.md)[]; `totalItems`: `number`; \}\>

#### Implementation of

[`ModelSecurityService`](../interfaces/ModelSecurityService.md).[`getFiles`](../interfaces/ModelSecurityService.md#getfiles)

***

### getGroup()

> **getGroup**(`uuid`): `Promise`\<[`ModelSecurityGroup`](../interfaces/ModelSecurityGroup.md)\>

Defined in: [src/airs/modelsecurity.ts:232](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/modelsecurity.ts#L232)

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<[`ModelSecurityGroup`](../interfaces/ModelSecurityGroup.md)\>

#### Implementation of

[`ModelSecurityService`](../interfaces/ModelSecurityService.md).[`getGroup`](../interfaces/ModelSecurityService.md#getgroup)

***

### getLabelKeys()

> **getLabelKeys**(`opts?`): `Promise`\<\{ `keys`: `string`[]; `totalItems`: `number`; \}\>

Defined in: [src/airs/modelsecurity.ts:482](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/modelsecurity.ts#L482)

#### Parameters

##### opts?

###### limit?

`number`

###### skip?

`number`

#### Returns

`Promise`\<\{ `keys`: `string`[]; `totalItems`: `number`; \}\>

#### Implementation of

[`ModelSecurityService`](../interfaces/ModelSecurityService.md).[`getLabelKeys`](../interfaces/ModelSecurityService.md#getlabelkeys)

***

### getLabelValues()

> **getLabelValues**(`key`, `opts?`): `Promise`\<\{ `totalItems`: `number`; `values`: `string`[]; \}\>

Defined in: [src/airs/modelsecurity.ts:497](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/modelsecurity.ts#L497)

#### Parameters

##### key

`string`

##### opts?

###### limit?

`number`

###### skip?

`number`

#### Returns

`Promise`\<\{ `totalItems`: `number`; `values`: `string`[]; \}\>

#### Implementation of

[`ModelSecurityService`](../interfaces/ModelSecurityService.md).[`getLabelValues`](../interfaces/ModelSecurityService.md#getlabelvalues)

***

### getModel()

> **getModel**(`uuid`): `Promise`\<`ModelSecurityModel`\>

Defined in: [src/airs/modelsecurity.ts:551](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/modelsecurity.ts#L551)

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<`ModelSecurityModel`\>

#### Implementation of

[`ModelSecurityService`](../interfaces/ModelSecurityService.md).[`getModel`](../interfaces/ModelSecurityService.md#getmodel)

***

### getModelVersion()

> **getModelVersion**(`uuid`): `Promise`\<`ModelSecurityModelVersion`\>

Defined in: [src/airs/modelsecurity.ts:576](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/modelsecurity.ts#L576)

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<`ModelSecurityModelVersion`\>

#### Implementation of

[`ModelSecurityService`](../interfaces/ModelSecurityService.md).[`getModelVersion`](../interfaces/ModelSecurityService.md#getmodelversion)

***

### getPyPIAuth()

> **getPyPIAuth**(): `Promise`\<[`ModelSecurityPyPIAuth`](../interfaces/ModelSecurityPyPIAuth.md)\>

Defined in: [src/airs/modelsecurity.ts:516](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/modelsecurity.ts#L516)

#### Returns

`Promise`\<[`ModelSecurityPyPIAuth`](../interfaces/ModelSecurityPyPIAuth.md)\>

#### Implementation of

[`ModelSecurityService`](../interfaces/ModelSecurityService.md).[`getPyPIAuth`](../interfaces/ModelSecurityService.md#getpypiauth)

***

### getRule()

> **getRule**(`uuid`): `Promise`\<[`ModelSecurityRule`](../interfaces/ModelSecurityRule.md)\>

Defined in: [src/airs/modelsecurity.ts:358](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/modelsecurity.ts#L358)

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<[`ModelSecurityRule`](../interfaces/ModelSecurityRule.md)\>

#### Implementation of

[`ModelSecurityService`](../interfaces/ModelSecurityService.md).[`getRule`](../interfaces/ModelSecurityService.md#getrule)

***

### getRuleInstance()

> **getRuleInstance**(`groupUuid`, `instanceUuid`): `Promise`\<[`ModelSecurityRuleInstance`](../interfaces/ModelSecurityRuleInstance.md)\>

Defined in: [src/airs/modelsecurity.ts:307](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/modelsecurity.ts#L307)

#### Parameters

##### groupUuid

`string`

##### instanceUuid

`string`

#### Returns

`Promise`\<[`ModelSecurityRuleInstance`](../interfaces/ModelSecurityRuleInstance.md)\>

#### Implementation of

[`ModelSecurityService`](../interfaces/ModelSecurityService.md).[`getRuleInstance`](../interfaces/ModelSecurityService.md#getruleinstance)

***

### getScan()

> **getScan**(`uuid`): `Promise`\<[`ModelSecurityScan`](../interfaces/ModelSecurityScan.md)\>

Defined in: [src/airs/modelsecurity.ts:394](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/modelsecurity.ts#L394)

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<[`ModelSecurityScan`](../interfaces/ModelSecurityScan.md)\>

#### Implementation of

[`ModelSecurityService`](../interfaces/ModelSecurityService.md).[`getScan`](../interfaces/ModelSecurityService.md#getscan)

***

### getViolation()

> **getViolation**(`uuid`): `Promise`\<[`ModelSecurityViolation`](../interfaces/ModelSecurityViolation.md)\>

Defined in: [src/airs/modelsecurity.ts:442](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/modelsecurity.ts#L442)

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<[`ModelSecurityViolation`](../interfaces/ModelSecurityViolation.md)\>

#### Implementation of

[`ModelSecurityService`](../interfaces/ModelSecurityService.md).[`getViolation`](../interfaces/ModelSecurityService.md#getviolation)

***

### getViolations()

> **getViolations**(`scanUuid`, `opts?`): `Promise`\<\{ `totalItems`: `number`; `violations`: [`ModelSecurityViolation`](../interfaces/ModelSecurityViolation.md)[]; \}\>

Defined in: [src/airs/modelsecurity.ts:427](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/modelsecurity.ts#L427)

#### Parameters

##### scanUuid

`string`

##### opts?

###### limit?

`number`

###### skip?

`number`

#### Returns

`Promise`\<\{ `totalItems`: `number`; `violations`: [`ModelSecurityViolation`](../interfaces/ModelSecurityViolation.md)[]; \}\>

#### Implementation of

[`ModelSecurityService`](../interfaces/ModelSecurityService.md).[`getViolations`](../interfaces/ModelSecurityService.md#getviolations)

***

### listGroups()

> **listGroups**(`opts?`): `Promise`\<\{ `groups`: [`ModelSecurityGroup`](../interfaces/ModelSecurityGroup.md)[]; `totalItems`: `number`; \}\>

Defined in: [src/airs/modelsecurity.ts:209](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/modelsecurity.ts#L209)

#### Parameters

##### opts?

[`ModelSecurityGroupListOptions`](../interfaces/ModelSecurityGroupListOptions.md)

#### Returns

`Promise`\<\{ `groups`: [`ModelSecurityGroup`](../interfaces/ModelSecurityGroup.md)[]; `totalItems`: `number`; \}\>

#### Implementation of

[`ModelSecurityService`](../interfaces/ModelSecurityService.md).[`listGroups`](../interfaces/ModelSecurityService.md#listgroups)

***

### listModels()

> **listModels**(`opts?`): `Promise`\<\{ `models`: `ModelSecurityModel`[]; `totalItems`: `number`; \}\>

Defined in: [src/airs/modelsecurity.ts:529](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/modelsecurity.ts#L529)

#### Parameters

##### opts?

`ModelSecurityModelListOptions`

#### Returns

`Promise`\<\{ `models`: `ModelSecurityModel`[]; `totalItems`: `number`; \}\>

#### Implementation of

[`ModelSecurityService`](../interfaces/ModelSecurityService.md).[`listModels`](../interfaces/ModelSecurityService.md#listmodels)

***

### listModelVersionFiles()

> **listModelVersionFiles**(`modelVersionUuid`, `opts?`): `Promise`\<\{ `files`: [`ModelSecurityFile`](../interfaces/ModelSecurityFile.md)[]; `totalItems`: `number`; \}\>

Defined in: [src/airs/modelsecurity.ts:581](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/modelsecurity.ts#L581)

#### Parameters

##### modelVersionUuid

`string`

##### opts?

###### limit?

`number`

###### skip?

`number`

#### Returns

`Promise`\<\{ `files`: [`ModelSecurityFile`](../interfaces/ModelSecurityFile.md)[]; `totalItems`: `number`; \}\>

#### Implementation of

[`ModelSecurityService`](../interfaces/ModelSecurityService.md).[`listModelVersionFiles`](../interfaces/ModelSecurityService.md#listmodelversionfiles)

***

### listModelVersions()

> **listModelVersions**(`modelUuid`, `opts?`): `Promise`\<\{ `totalItems`: `number`; `versions`: `ModelSecurityModelVersion`[]; \}\>

Defined in: [src/airs/modelsecurity.ts:556](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/modelsecurity.ts#L556)

#### Parameters

##### modelUuid

`string`

##### opts?

`ModelSecurityModelVersionListOptions`

#### Returns

`Promise`\<\{ `totalItems`: `number`; `versions`: `ModelSecurityModelVersion`[]; \}\>

#### Implementation of

[`ModelSecurityService`](../interfaces/ModelSecurityService.md).[`listModelVersions`](../interfaces/ModelSecurityService.md#listmodelversions)

***

### listRuleInstances()

> **listRuleInstances**(`groupUuid`, `opts?`): `Promise`\<\{ `ruleInstances`: [`ModelSecurityRuleInstance`](../interfaces/ModelSecurityRuleInstance.md)[]; `totalItems`: `number`; \}\>

Defined in: [src/airs/modelsecurity.ts:283](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/modelsecurity.ts#L283)

#### Parameters

##### groupUuid

`string`

##### opts?

[`ModelSecurityRuleInstanceListOptions`](../interfaces/ModelSecurityRuleInstanceListOptions.md)

#### Returns

`Promise`\<\{ `ruleInstances`: [`ModelSecurityRuleInstance`](../interfaces/ModelSecurityRuleInstance.md)[]; `totalItems`: `number`; \}\>

#### Implementation of

[`ModelSecurityService`](../interfaces/ModelSecurityService.md).[`listRuleInstances`](../interfaces/ModelSecurityService.md#listruleinstances)

***

### listRules()

> **listRules**(`opts?`): `Promise`\<\{ `rules`: [`ModelSecurityRule`](../interfaces/ModelSecurityRule.md)[]; `totalItems`: `number`; \}\>

Defined in: [src/airs/modelsecurity.ts:338](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/modelsecurity.ts#L338)

#### Parameters

##### opts?

[`ModelSecurityRuleListOptions`](../interfaces/ModelSecurityRuleListOptions.md)

#### Returns

`Promise`\<\{ `rules`: [`ModelSecurityRule`](../interfaces/ModelSecurityRule.md)[]; `totalItems`: `number`; \}\>

#### Implementation of

[`ModelSecurityService`](../interfaces/ModelSecurityService.md).[`listRules`](../interfaces/ModelSecurityService.md#listrules)

***

### listScans()

> **listScans**(`opts?`): `Promise`\<\{ `scans`: [`ModelSecurityScan`](../interfaces/ModelSecurityScan.md)[]; `totalItems`: `number`; \}\>

Defined in: [src/airs/modelsecurity.ts:372](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/modelsecurity.ts#L372)

#### Parameters

##### opts?

[`ModelSecurityScanListOptions`](../interfaces/ModelSecurityScanListOptions.md)

#### Returns

`Promise`\<\{ `scans`: [`ModelSecurityScan`](../interfaces/ModelSecurityScan.md)[]; `totalItems`: `number`; \}\>

#### Implementation of

[`ModelSecurityService`](../interfaces/ModelSecurityService.md).[`listScans`](../interfaces/ModelSecurityService.md#listscans)

***

### setLabels()

> **setLabels**(`scanUuid`, `labels`): `Promise`\<`void`\>

Defined in: [src/airs/modelsecurity.ts:474](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/modelsecurity.ts#L474)

#### Parameters

##### scanUuid

`string`

##### labels

[`ModelSecurityLabel`](../interfaces/ModelSecurityLabel.md)[]

#### Returns

`Promise`\<`void`\>

#### Implementation of

[`ModelSecurityService`](../interfaces/ModelSecurityService.md).[`setLabels`](../interfaces/ModelSecurityService.md#setlabels)

***

### updateGroup()

> **updateGroup**(`uuid`, `request`): `Promise`\<[`ModelSecurityGroup`](../interfaces/ModelSecurityGroup.md)\>

Defined in: [src/airs/modelsecurity.ts:249](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/modelsecurity.ts#L249)

#### Parameters

##### uuid

`string`

##### request

[`ModelSecurityGroupUpdateRequest`](../interfaces/ModelSecurityGroupUpdateRequest.md)

#### Returns

`Promise`\<[`ModelSecurityGroup`](../interfaces/ModelSecurityGroup.md)\>

#### Implementation of

[`ModelSecurityService`](../interfaces/ModelSecurityService.md).[`updateGroup`](../interfaces/ModelSecurityService.md#updategroup)

***

### updateRuleInstance()

> **updateRuleInstance**(`groupUuid`, `instanceUuid`, `request`): `Promise`\<[`ModelSecurityRuleInstance`](../interfaces/ModelSecurityRuleInstance.md)\>

Defined in: [src/airs/modelsecurity.ts:315](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/modelsecurity.ts#L315)

#### Parameters

##### groupUuid

`string`

##### instanceUuid

`string`

##### request

[`ModelSecurityRuleInstanceUpdateRequest`](../interfaces/ModelSecurityRuleInstanceUpdateRequest.md)

#### Returns

`Promise`\<[`ModelSecurityRuleInstance`](../interfaces/ModelSecurityRuleInstance.md)\>

#### Implementation of

[`ModelSecurityService`](../interfaces/ModelSecurityService.md).[`updateRuleInstance`](../interfaces/ModelSecurityService.md#updateruleinstance)
