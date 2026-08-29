# Interface: ModelSecurityService

Defined in: src/airs/types.ts:977

Contract for Model Security operations.

## Methods

### addLabels()

> **addLabels**(`scanUuid`, `labels`): `Promise`\<`void`\>

Defined in: src/airs/types.ts:1034

#### Parameters

##### scanUuid

`string`

##### labels

[`ModelSecurityLabel`](ModelSecurityLabel.md)[]

#### Returns

`Promise`\<`void`\>

***

### createGroup()

> **createGroup**(`request`): `Promise`\<[`ModelSecurityGroup`](ModelSecurityGroup.md)\>

Defined in: src/airs/types.ts:985

#### Parameters

##### request

[`ModelSecurityGroupCreateRequest`](ModelSecurityGroupCreateRequest.md)

#### Returns

`Promise`\<[`ModelSecurityGroup`](ModelSecurityGroup.md)\>

***

### createScan()

> **createScan**(`request`): `Promise`\<[`ModelSecurityScan`](ModelSecurityScan.md)\>

Defined in: src/airs/types.ts:1008

#### Parameters

##### request

`Record`\<`string`, `unknown`\>

#### Returns

`Promise`\<[`ModelSecurityScan`](ModelSecurityScan.md)\>

***

### deleteGroup()

> **deleteGroup**(`uuid`): `Promise`\<`void`\>

Defined in: src/airs/types.ts:987

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<`void`\>

***

### deleteLabels()

> **deleteLabels**(`scanUuid`, `keys`): `Promise`\<`void`\>

Defined in: src/airs/types.ts:1036

#### Parameters

##### scanUuid

`string`

##### keys

`string`[]

#### Returns

`Promise`\<`void`\>

***

### getEvaluation()

> **getEvaluation**(`uuid`): `Promise`\<[`ModelSecurityEvaluation`](ModelSecurityEvaluation.md)\>

Defined in: src/airs/types.ts:1021

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<[`ModelSecurityEvaluation`](ModelSecurityEvaluation.md)\>

***

### getEvaluations()

> **getEvaluations**(`scanUuid`, `opts?`): `Promise`\<\{ `evaluations`: [`ModelSecurityEvaluation`](ModelSecurityEvaluation.md)[]; `totalItems`: `number`; \}\>

Defined in: src/airs/types.ts:1017

#### Parameters

##### scanUuid

`string`

##### opts?

###### limit?

`number`

###### skip?

`number`

#### Returns

`Promise`\<\{ `evaluations`: [`ModelSecurityEvaluation`](ModelSecurityEvaluation.md)[]; `totalItems`: `number`; \}\>

***

### getFiles()

> **getFiles**(`scanUuid`, `opts?`): `Promise`\<\{ `files`: [`ModelSecurityFile`](ModelSecurityFile.md)[]; `totalItems`: `number`; \}\>

Defined in: src/airs/types.ts:1029

#### Parameters

##### scanUuid

`string`

##### opts?

[`ModelSecurityFileListOptions`](ModelSecurityFileListOptions.md)

#### Returns

`Promise`\<\{ `files`: [`ModelSecurityFile`](ModelSecurityFile.md)[]; `totalItems`: `number`; \}\>

***

### getGroup()

> **getGroup**(`uuid`): `Promise`\<[`ModelSecurityGroup`](ModelSecurityGroup.md)\>

Defined in: src/airs/types.ts:984

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<[`ModelSecurityGroup`](ModelSecurityGroup.md)\>

***

### getLabelKeys()

> **getLabelKeys**(`opts?`): `Promise`\<\{ `keys`: `string`[]; `totalItems`: `number`; \}\>

Defined in: src/airs/types.ts:1037

#### Parameters

##### opts?

###### limit?

`number`

###### skip?

`number`

#### Returns

`Promise`\<\{ `keys`: `string`[]; `totalItems`: `number`; \}\>

***

### getLabelValues()

> **getLabelValues**(`key`, `opts?`): `Promise`\<\{ `totalItems`: `number`; `values`: `string`[]; \}\>

Defined in: src/airs/types.ts:1041

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

***

### getModel()

> **getModel**(`uuid`): `Promise`\<[`ModelSecurityModel`](ModelSecurityModel.md)\>

Defined in: src/airs/types.ts:1054

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<[`ModelSecurityModel`](ModelSecurityModel.md)\>

***

### getModelVersion()

> **getModelVersion**(`uuid`): `Promise`\<[`ModelSecurityModelVersion`](ModelSecurityModelVersion.md)\>

Defined in: src/airs/types.ts:1059

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<[`ModelSecurityModelVersion`](ModelSecurityModelVersion.md)\>

***

### getPyPIAuth()

> **getPyPIAuth**(): `Promise`\<[`ModelSecurityPyPIAuth`](ModelSecurityPyPIAuth.md)\>

Defined in: src/airs/types.ts:1046

#### Returns

`Promise`\<[`ModelSecurityPyPIAuth`](ModelSecurityPyPIAuth.md)\>

***

### getRule()

> **getRule**(`uuid`): `Promise`\<[`ModelSecurityRule`](ModelSecurityRule.md)\>

Defined in: src/airs/types.ts:1006

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<[`ModelSecurityRule`](ModelSecurityRule.md)\>

***

### getRuleInstance()

> **getRuleInstance**(`groupUuid`, `instanceUuid`): `Promise`\<[`ModelSecurityRuleInstance`](ModelSecurityRuleInstance.md)\>

Defined in: src/airs/types.ts:993

#### Parameters

##### groupUuid

`string`

##### instanceUuid

`string`

#### Returns

`Promise`\<[`ModelSecurityRuleInstance`](ModelSecurityRuleInstance.md)\>

***

### getScan()

> **getScan**(`uuid`): `Promise`\<[`ModelSecurityScan`](ModelSecurityScan.md)\>

Defined in: src/airs/types.ts:1015

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<[`ModelSecurityScan`](ModelSecurityScan.md)\>

***

### getViolation()

> **getViolation**(`uuid`): `Promise`\<[`ModelSecurityViolation`](ModelSecurityViolation.md)\>

Defined in: src/airs/types.ts:1027

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<[`ModelSecurityViolation`](ModelSecurityViolation.md)\>

***

### getViolations()

> **getViolations**(`scanUuid`, `opts?`): `Promise`\<\{ `totalItems`: `number`; `violations`: [`ModelSecurityViolation`](ModelSecurityViolation.md)[]; \}\>

Defined in: src/airs/types.ts:1023

#### Parameters

##### scanUuid

`string`

##### opts?

###### limit?

`number`

###### skip?

`number`

#### Returns

`Promise`\<\{ `totalItems`: `number`; `violations`: [`ModelSecurityViolation`](ModelSecurityViolation.md)[]; \}\>

***

### listAllGroups()

> **listAllGroups**(`opts?`): `Promise`\<[`ModelSecurityGroup`](ModelSecurityGroup.md)[]\>

Defined in: src/airs/types.ts:981

#### Parameters

##### opts?

[`ModelSecurityGroupListOptions`](ModelSecurityGroupListOptions.md) & `object`

#### Returns

`Promise`\<[`ModelSecurityGroup`](ModelSecurityGroup.md)[]\>

***

### listAllModels()

> **listAllModels**(`opts?`): `Promise`\<[`ModelSecurityModel`](ModelSecurityModel.md)[]\>

Defined in: src/airs/types.ts:1051

#### Parameters

##### opts?

[`ModelSecurityModelListOptions`](ModelSecurityModelListOptions.md) & `object`

#### Returns

`Promise`\<[`ModelSecurityModel`](ModelSecurityModel.md)[]\>

***

### listAllRules()

> **listAllRules**(`opts?`): `Promise`\<[`ModelSecurityRule`](ModelSecurityRule.md)[]\>

Defined in: src/airs/types.ts:1003

#### Parameters

##### opts?

[`ModelSecurityRuleListOptions`](ModelSecurityRuleListOptions.md) & `object`

#### Returns

`Promise`\<[`ModelSecurityRule`](ModelSecurityRule.md)[]\>

***

### listAllScans()

> **listAllScans**(`opts?`): `Promise`\<[`ModelSecurityScan`](ModelSecurityScan.md)[]\>

Defined in: src/airs/types.ts:1012

#### Parameters

##### opts?

[`ModelSecurityScanListOptions`](ModelSecurityScanListOptions.md) & `object`

#### Returns

`Promise`\<[`ModelSecurityScan`](ModelSecurityScan.md)[]\>

***

### listGroups()

> **listGroups**(`opts?`): `Promise`\<\{ `groups`: [`ModelSecurityGroup`](ModelSecurityGroup.md)[]; `totalItems`: `number`; \}\>

Defined in: src/airs/types.ts:978

#### Parameters

##### opts?

[`ModelSecurityGroupListOptions`](ModelSecurityGroupListOptions.md)

#### Returns

`Promise`\<\{ `groups`: [`ModelSecurityGroup`](ModelSecurityGroup.md)[]; `totalItems`: `number`; \}\>

***

### listModels()

> **listModels**(`opts?`): `Promise`\<\{ `models`: [`ModelSecurityModel`](ModelSecurityModel.md)[]; `totalItems`: `number`; \}\>

Defined in: src/airs/types.ts:1048

#### Parameters

##### opts?

[`ModelSecurityModelListOptions`](ModelSecurityModelListOptions.md)

#### Returns

`Promise`\<\{ `models`: [`ModelSecurityModel`](ModelSecurityModel.md)[]; `totalItems`: `number`; \}\>

***

### listModelVersionFiles()

> **listModelVersionFiles**(`modelVersionUuid`, `opts?`): `Promise`\<\{ `files`: [`ModelSecurityFile`](ModelSecurityFile.md)[]; `totalItems`: `number`; \}\>

Defined in: src/airs/types.ts:1060

#### Parameters

##### modelVersionUuid

`string`

##### opts?

###### limit?

`number`

###### skip?

`number`

#### Returns

`Promise`\<\{ `files`: [`ModelSecurityFile`](ModelSecurityFile.md)[]; `totalItems`: `number`; \}\>

***

### listModelVersions()

> **listModelVersions**(`modelUuid`, `opts?`): `Promise`\<\{ `totalItems`: `number`; `versions`: [`ModelSecurityModelVersion`](ModelSecurityModelVersion.md)[]; \}\>

Defined in: src/airs/types.ts:1055

#### Parameters

##### modelUuid

`string`

##### opts?

[`ModelSecurityModelVersionListOptions`](ModelSecurityModelVersionListOptions.md)

#### Returns

`Promise`\<\{ `totalItems`: `number`; `versions`: [`ModelSecurityModelVersion`](ModelSecurityModelVersion.md)[]; \}\>

***

### listRuleInstances()

> **listRuleInstances**(`groupUuid`, `opts?`): `Promise`\<\{ `ruleInstances`: [`ModelSecurityRuleInstance`](ModelSecurityRuleInstance.md)[]; `totalItems`: `number`; \}\>

Defined in: src/airs/types.ts:989

#### Parameters

##### groupUuid

`string`

##### opts?

[`ModelSecurityRuleInstanceListOptions`](ModelSecurityRuleInstanceListOptions.md)

#### Returns

`Promise`\<\{ `ruleInstances`: [`ModelSecurityRuleInstance`](ModelSecurityRuleInstance.md)[]; `totalItems`: `number`; \}\>

***

### listRules()

> **listRules**(`opts?`): `Promise`\<\{ `rules`: [`ModelSecurityRule`](ModelSecurityRule.md)[]; `totalItems`: `number`; \}\>

Defined in: src/airs/types.ts:1000

#### Parameters

##### opts?

[`ModelSecurityRuleListOptions`](ModelSecurityRuleListOptions.md)

#### Returns

`Promise`\<\{ `rules`: [`ModelSecurityRule`](ModelSecurityRule.md)[]; `totalItems`: `number`; \}\>

***

### listScans()

> **listScans**(`opts?`): `Promise`\<\{ `scans`: [`ModelSecurityScan`](ModelSecurityScan.md)[]; `totalItems`: `number`; \}\>

Defined in: src/airs/types.ts:1009

#### Parameters

##### opts?

[`ModelSecurityScanListOptions`](ModelSecurityScanListOptions.md)

#### Returns

`Promise`\<\{ `scans`: [`ModelSecurityScan`](ModelSecurityScan.md)[]; `totalItems`: `number`; \}\>

***

### setLabels()

> **setLabels**(`scanUuid`, `labels`): `Promise`\<`void`\>

Defined in: src/airs/types.ts:1035

#### Parameters

##### scanUuid

`string`

##### labels

[`ModelSecurityLabel`](ModelSecurityLabel.md)[]

#### Returns

`Promise`\<`void`\>

***

### updateGroup()

> **updateGroup**(`uuid`, `request`): `Promise`\<[`ModelSecurityGroup`](ModelSecurityGroup.md)\>

Defined in: src/airs/types.ts:986

#### Parameters

##### uuid

`string`

##### request

[`ModelSecurityGroupUpdateRequest`](ModelSecurityGroupUpdateRequest.md)

#### Returns

`Promise`\<[`ModelSecurityGroup`](ModelSecurityGroup.md)\>

***

### updateRuleInstance()

> **updateRuleInstance**(`groupUuid`, `instanceUuid`, `request`): `Promise`\<[`ModelSecurityRuleInstance`](ModelSecurityRuleInstance.md)\>

Defined in: src/airs/types.ts:994

#### Parameters

##### groupUuid

`string`

##### instanceUuid

`string`

##### request

[`ModelSecurityRuleInstanceUpdateRequest`](ModelSecurityRuleInstanceUpdateRequest.md)

#### Returns

`Promise`\<[`ModelSecurityRuleInstance`](ModelSecurityRuleInstance.md)\>
