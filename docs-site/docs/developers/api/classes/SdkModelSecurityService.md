# Class: SdkModelSecurityService

Defined in: src/airs/modelsecurity.ts:198

Wraps the SDK's ModelSecurityClient to implement ModelSecurityService.
Provides security group CRUD, rule browsing, scan operations, and label management.

## Implements

- [`ModelSecurityService`](../interfaces/ModelSecurityService.md)

## Constructors

### Constructor

> **new SdkModelSecurityService**(`opts?`): `SdkModelSecurityService`

Defined in: src/airs/modelsecurity.ts:201

#### Parameters

##### opts?

`ModelSecurityClientOptions`

#### Returns

`SdkModelSecurityService`

## Methods

### addLabels()

> **addLabels**(`scanUuid`, `labels`): `Promise`\<`void`\>

Defined in: src/airs/modelsecurity.ts:510

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

Defined in: src/airs/modelsecurity.ts:252

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

Defined in: src/airs/modelsecurity.ts:394

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

Defined in: src/airs/modelsecurity.ts:272

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

Defined in: src/airs/modelsecurity.ts:284

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

Defined in: src/airs/modelsecurity.ts:518

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

Defined in: src/airs/modelsecurity.ts:458

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

Defined in: src/airs/modelsecurity.ts:443

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

Defined in: src/airs/modelsecurity.ts:491

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

Defined in: src/airs/modelsecurity.ts:247

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

Defined in: src/airs/modelsecurity.ts:522

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

Defined in: src/airs/modelsecurity.ts:537

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

> **getModel**(`uuid`): `Promise`\<[`ModelSecurityModel`](../interfaces/ModelSecurityModel.md)\>

Defined in: src/airs/modelsecurity.ts:605

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<[`ModelSecurityModel`](../interfaces/ModelSecurityModel.md)\>

#### Implementation of

[`ModelSecurityService`](../interfaces/ModelSecurityService.md).[`getModel`](../interfaces/ModelSecurityService.md#getmodel)

***

### getModelVersion()

> **getModelVersion**(`uuid`): `Promise`\<[`ModelSecurityModelVersion`](../interfaces/ModelSecurityModelVersion.md)\>

Defined in: src/airs/modelsecurity.ts:630

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<[`ModelSecurityModelVersion`](../interfaces/ModelSecurityModelVersion.md)\>

#### Implementation of

[`ModelSecurityService`](../interfaces/ModelSecurityService.md).[`getModelVersion`](../interfaces/ModelSecurityService.md#getmodelversion)

***

### getPyPIAuth()

> **getPyPIAuth**(): `Promise`\<[`ModelSecurityPyPIAuth`](../interfaces/ModelSecurityPyPIAuth.md)\>

Defined in: src/airs/modelsecurity.ts:556

#### Returns

`Promise`\<[`ModelSecurityPyPIAuth`](../interfaces/ModelSecurityPyPIAuth.md)\>

#### Implementation of

[`ModelSecurityService`](../interfaces/ModelSecurityService.md).[`getPyPIAuth`](../interfaces/ModelSecurityService.md#getpypiauth)

***

### getRule()

> **getRule**(`uuid`): `Promise`\<[`ModelSecurityRule`](../interfaces/ModelSecurityRule.md)\>

Defined in: src/airs/modelsecurity.ts:385

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

Defined in: src/airs/modelsecurity.ts:322

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

Defined in: src/airs/modelsecurity.ts:434

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

Defined in: src/airs/modelsecurity.ts:482

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

Defined in: src/airs/modelsecurity.ts:467

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

### listAllGroups()

> **listAllGroups**(`opts?`): `Promise`\<[`ModelSecurityGroup`](../interfaces/ModelSecurityGroup.md)[]\>

Defined in: src/airs/modelsecurity.ts:232

#### Parameters

##### opts?

[`ModelSecurityGroupListOptions`](../interfaces/ModelSecurityGroupListOptions.md) & `object` = `{}`

#### Returns

`Promise`\<[`ModelSecurityGroup`](../interfaces/ModelSecurityGroup.md)[]\>

#### Implementation of

[`ModelSecurityService`](../interfaces/ModelSecurityService.md).[`listAllGroups`](../interfaces/ModelSecurityService.md#listallgroups)

***

### listAllModels()

> **listAllModels**(`opts?`): `Promise`\<[`ModelSecurityModel`](../interfaces/ModelSecurityModel.md)[]\>

Defined in: src/airs/modelsecurity.ts:591

#### Parameters

##### opts?

[`ModelSecurityModelListOptions`](../interfaces/ModelSecurityModelListOptions.md) & `object` = `{}`

#### Returns

`Promise`\<[`ModelSecurityModel`](../interfaces/ModelSecurityModel.md)[]\>

#### Implementation of

[`ModelSecurityService`](../interfaces/ModelSecurityService.md).[`listAllModels`](../interfaces/ModelSecurityService.md#listallmodels)

***

### listAllRules()

> **listAllRules**(`opts?`): `Promise`\<[`ModelSecurityRule`](../interfaces/ModelSecurityRule.md)[]\>

Defined in: src/airs/modelsecurity.ts:373

#### Parameters

##### opts?

[`ModelSecurityRuleListOptions`](../interfaces/ModelSecurityRuleListOptions.md) & `object` = `{}`

#### Returns

`Promise`\<[`ModelSecurityRule`](../interfaces/ModelSecurityRule.md)[]\>

#### Implementation of

[`ModelSecurityService`](../interfaces/ModelSecurityService.md).[`listAllRules`](../interfaces/ModelSecurityService.md#listallrules)

***

### listAllScans()

> **listAllScans**(`opts?`): `Promise`\<[`ModelSecurityScan`](../interfaces/ModelSecurityScan.md)[]\>

Defined in: src/airs/modelsecurity.ts:421

#### Parameters

##### opts?

[`ModelSecurityScanListOptions`](../interfaces/ModelSecurityScanListOptions.md) & `object` = `{}`

#### Returns

`Promise`\<[`ModelSecurityScan`](../interfaces/ModelSecurityScan.md)[]\>

#### Implementation of

[`ModelSecurityService`](../interfaces/ModelSecurityService.md).[`listAllScans`](../interfaces/ModelSecurityService.md#listallscans)

***

### listGroups()

> **listGroups**(`opts?`): `Promise`\<\{ `groups`: [`ModelSecurityGroup`](../interfaces/ModelSecurityGroup.md)[]; `totalItems`: `number`; \}\>

Defined in: src/airs/modelsecurity.ts:209

#### Parameters

##### opts?

[`ModelSecurityGroupListOptions`](../interfaces/ModelSecurityGroupListOptions.md)

#### Returns

`Promise`\<\{ `groups`: [`ModelSecurityGroup`](../interfaces/ModelSecurityGroup.md)[]; `totalItems`: `number`; \}\>

#### Implementation of

[`ModelSecurityService`](../interfaces/ModelSecurityService.md).[`listGroups`](../interfaces/ModelSecurityService.md#listgroups)

***

### listModels()

> **listModels**(`opts?`): `Promise`\<\{ `models`: [`ModelSecurityModel`](../interfaces/ModelSecurityModel.md)[]; `totalItems`: `number`; \}\>

Defined in: src/airs/modelsecurity.ts:569

#### Parameters

##### opts?

[`ModelSecurityModelListOptions`](../interfaces/ModelSecurityModelListOptions.md)

#### Returns

`Promise`\<\{ `models`: [`ModelSecurityModel`](../interfaces/ModelSecurityModel.md)[]; `totalItems`: `number`; \}\>

#### Implementation of

[`ModelSecurityService`](../interfaces/ModelSecurityService.md).[`listModels`](../interfaces/ModelSecurityService.md#listmodels)

***

### listModelVersionFiles()

> **listModelVersionFiles**(`modelVersionUuid`, `opts?`): `Promise`\<\{ `files`: [`ModelSecurityFile`](../interfaces/ModelSecurityFile.md)[]; `totalItems`: `number`; \}\>

Defined in: src/airs/modelsecurity.ts:635

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

> **listModelVersions**(`modelUuid`, `opts?`): `Promise`\<\{ `totalItems`: `number`; `versions`: [`ModelSecurityModelVersion`](../interfaces/ModelSecurityModelVersion.md)[]; \}\>

Defined in: src/airs/modelsecurity.ts:610

#### Parameters

##### modelUuid

`string`

##### opts?

[`ModelSecurityModelVersionListOptions`](../interfaces/ModelSecurityModelVersionListOptions.md)

#### Returns

`Promise`\<\{ `totalItems`: `number`; `versions`: [`ModelSecurityModelVersion`](../interfaces/ModelSecurityModelVersion.md)[]; \}\>

#### Implementation of

[`ModelSecurityService`](../interfaces/ModelSecurityService.md).[`listModelVersions`](../interfaces/ModelSecurityService.md#listmodelversions)

***

### listRuleInstances()

> **listRuleInstances**(`groupUuid`, `opts?`): `Promise`\<\{ `ruleInstances`: [`ModelSecurityRuleInstance`](../interfaces/ModelSecurityRuleInstance.md)[]; `totalItems`: `number`; \}\>

Defined in: src/airs/modelsecurity.ts:298

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

Defined in: src/airs/modelsecurity.ts:353

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

Defined in: src/airs/modelsecurity.ts:399

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

Defined in: src/airs/modelsecurity.ts:514

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

Defined in: src/airs/modelsecurity.ts:264

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

Defined in: src/airs/modelsecurity.ts:330

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
