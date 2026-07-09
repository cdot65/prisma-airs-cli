# Interface: ModelSecurityService

Defined in: [src/airs/types.ts:887](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L887)

Contract for Model Security operations.

## Methods

### addLabels()

> **addLabels**(`scanUuid`, `labels`): `Promise`\<`void`\>

Defined in: [src/airs/types.ts:935](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L935)

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

Defined in: [src/airs/types.ts:892](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L892)

#### Parameters

##### request

[`ModelSecurityGroupCreateRequest`](ModelSecurityGroupCreateRequest.md)

#### Returns

`Promise`\<[`ModelSecurityGroup`](ModelSecurityGroup.md)\>

***

### createScan()

> **createScan**(`request`): `Promise`\<[`ModelSecurityScan`](ModelSecurityScan.md)\>

Defined in: [src/airs/types.ts:912](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L912)

#### Parameters

##### request

`Record`\<`string`, `unknown`\>

#### Returns

`Promise`\<[`ModelSecurityScan`](ModelSecurityScan.md)\>

***

### deleteGroup()

> **deleteGroup**(`uuid`): `Promise`\<`void`\>

Defined in: [src/airs/types.ts:894](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L894)

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<`void`\>

***

### deleteLabels()

> **deleteLabels**(`scanUuid`, `keys`): `Promise`\<`void`\>

Defined in: [src/airs/types.ts:937](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L937)

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

Defined in: [src/airs/types.ts:922](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L922)

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<[`ModelSecurityEvaluation`](ModelSecurityEvaluation.md)\>

***

### getEvaluations()

> **getEvaluations**(`scanUuid`, `opts?`): `Promise`\<\{ `evaluations`: [`ModelSecurityEvaluation`](ModelSecurityEvaluation.md)[]; `totalItems`: `number`; \}\>

Defined in: [src/airs/types.ts:918](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L918)

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

Defined in: [src/airs/types.ts:930](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L930)

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

Defined in: [src/airs/types.ts:891](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L891)

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<[`ModelSecurityGroup`](ModelSecurityGroup.md)\>

***

### getLabelKeys()

> **getLabelKeys**(`opts?`): `Promise`\<\{ `keys`: `string`[]; `totalItems`: `number`; \}\>

Defined in: [src/airs/types.ts:938](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L938)

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

Defined in: [src/airs/types.ts:942](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L942)

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

> **getModel**(`uuid`): `Promise`\<`ModelSecurityModel`\>

Defined in: [src/airs/types.ts:952](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L952)

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<`ModelSecurityModel`\>

***

### getModelVersion()

> **getModelVersion**(`uuid`): `Promise`\<`ModelSecurityModelVersion`\>

Defined in: [src/airs/types.ts:957](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L957)

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<`ModelSecurityModelVersion`\>

***

### getPyPIAuth()

> **getPyPIAuth**(): `Promise`\<[`ModelSecurityPyPIAuth`](ModelSecurityPyPIAuth.md)\>

Defined in: [src/airs/types.ts:947](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L947)

#### Returns

`Promise`\<[`ModelSecurityPyPIAuth`](ModelSecurityPyPIAuth.md)\>

***

### getRule()

> **getRule**(`uuid`): `Promise`\<[`ModelSecurityRule`](ModelSecurityRule.md)\>

Defined in: [src/airs/types.ts:910](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L910)

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<[`ModelSecurityRule`](ModelSecurityRule.md)\>

***

### getRuleInstance()

> **getRuleInstance**(`groupUuid`, `instanceUuid`): `Promise`\<[`ModelSecurityRuleInstance`](ModelSecurityRuleInstance.md)\>

Defined in: [src/airs/types.ts:900](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L900)

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

Defined in: [src/airs/types.ts:916](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L916)

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<[`ModelSecurityScan`](ModelSecurityScan.md)\>

***

### getViolation()

> **getViolation**(`uuid`): `Promise`\<[`ModelSecurityViolation`](ModelSecurityViolation.md)\>

Defined in: [src/airs/types.ts:928](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L928)

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<[`ModelSecurityViolation`](ModelSecurityViolation.md)\>

***

### getViolations()

> **getViolations**(`scanUuid`, `opts?`): `Promise`\<\{ `totalItems`: `number`; `violations`: [`ModelSecurityViolation`](ModelSecurityViolation.md)[]; \}\>

Defined in: [src/airs/types.ts:924](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L924)

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

### listGroups()

> **listGroups**(`opts?`): `Promise`\<\{ `groups`: [`ModelSecurityGroup`](ModelSecurityGroup.md)[]; `totalItems`: `number`; \}\>

Defined in: [src/airs/types.ts:888](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L888)

#### Parameters

##### opts?

[`ModelSecurityGroupListOptions`](ModelSecurityGroupListOptions.md)

#### Returns

`Promise`\<\{ `groups`: [`ModelSecurityGroup`](ModelSecurityGroup.md)[]; `totalItems`: `number`; \}\>

***

### listModels()

> **listModels**(`opts?`): `Promise`\<\{ `models`: `ModelSecurityModel`[]; `totalItems`: `number`; \}\>

Defined in: [src/airs/types.ts:949](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L949)

#### Parameters

##### opts?

`ModelSecurityModelListOptions`

#### Returns

`Promise`\<\{ `models`: `ModelSecurityModel`[]; `totalItems`: `number`; \}\>

***

### listModelVersionFiles()

> **listModelVersionFiles**(`modelVersionUuid`, `opts?`): `Promise`\<\{ `files`: [`ModelSecurityFile`](ModelSecurityFile.md)[]; `totalItems`: `number`; \}\>

Defined in: [src/airs/types.ts:958](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L958)

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

> **listModelVersions**(`modelUuid`, `opts?`): `Promise`\<\{ `totalItems`: `number`; `versions`: `ModelSecurityModelVersion`[]; \}\>

Defined in: [src/airs/types.ts:953](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L953)

#### Parameters

##### modelUuid

`string`

##### opts?

`ModelSecurityModelVersionListOptions`

#### Returns

`Promise`\<\{ `totalItems`: `number`; `versions`: `ModelSecurityModelVersion`[]; \}\>

***

### listRuleInstances()

> **listRuleInstances**(`groupUuid`, `opts?`): `Promise`\<\{ `ruleInstances`: [`ModelSecurityRuleInstance`](ModelSecurityRuleInstance.md)[]; `totalItems`: `number`; \}\>

Defined in: [src/airs/types.ts:896](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L896)

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

Defined in: [src/airs/types.ts:907](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L907)

#### Parameters

##### opts?

[`ModelSecurityRuleListOptions`](ModelSecurityRuleListOptions.md)

#### Returns

`Promise`\<\{ `rules`: [`ModelSecurityRule`](ModelSecurityRule.md)[]; `totalItems`: `number`; \}\>

***

### listScans()

> **listScans**(`opts?`): `Promise`\<\{ `scans`: [`ModelSecurityScan`](ModelSecurityScan.md)[]; `totalItems`: `number`; \}\>

Defined in: [src/airs/types.ts:913](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L913)

#### Parameters

##### opts?

[`ModelSecurityScanListOptions`](ModelSecurityScanListOptions.md)

#### Returns

`Promise`\<\{ `scans`: [`ModelSecurityScan`](ModelSecurityScan.md)[]; `totalItems`: `number`; \}\>

***

### setLabels()

> **setLabels**(`scanUuid`, `labels`): `Promise`\<`void`\>

Defined in: [src/airs/types.ts:936](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L936)

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

Defined in: [src/airs/types.ts:893](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L893)

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

Defined in: [src/airs/types.ts:901](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L901)

#### Parameters

##### groupUuid

`string`

##### instanceUuid

`string`

##### request

[`ModelSecurityRuleInstanceUpdateRequest`](ModelSecurityRuleInstanceUpdateRequest.md)

#### Returns

`Promise`\<[`ModelSecurityRuleInstance`](ModelSecurityRuleInstance.md)\>
