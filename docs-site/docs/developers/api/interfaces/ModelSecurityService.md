# Interface: ModelSecurityService

Defined in: [src/airs/types.ts:965](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L965)

Contract for Model Security operations.

## Methods

### addLabels()

> **addLabels**(`scanUuid`, `labels`): `Promise`\<`void`\>

Defined in: [src/airs/types.ts:1013](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L1013)

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

Defined in: [src/airs/types.ts:970](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L970)

#### Parameters

##### request

[`ModelSecurityGroupCreateRequest`](ModelSecurityGroupCreateRequest.md)

#### Returns

`Promise`\<[`ModelSecurityGroup`](ModelSecurityGroup.md)\>

***

### createScan()

> **createScan**(`request`): `Promise`\<[`ModelSecurityScan`](ModelSecurityScan.md)\>

Defined in: [src/airs/types.ts:990](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L990)

#### Parameters

##### request

`Record`\<`string`, `unknown`\>

#### Returns

`Promise`\<[`ModelSecurityScan`](ModelSecurityScan.md)\>

***

### deleteGroup()

> **deleteGroup**(`uuid`): `Promise`\<`void`\>

Defined in: [src/airs/types.ts:972](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L972)

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<`void`\>

***

### deleteLabels()

> **deleteLabels**(`scanUuid`, `keys`): `Promise`\<`void`\>

Defined in: [src/airs/types.ts:1015](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L1015)

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

Defined in: [src/airs/types.ts:1000](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L1000)

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<[`ModelSecurityEvaluation`](ModelSecurityEvaluation.md)\>

***

### getEvaluations()

> **getEvaluations**(`scanUuid`, `opts?`): `Promise`\<\{ `evaluations`: [`ModelSecurityEvaluation`](ModelSecurityEvaluation.md)[]; `totalItems`: `number`; \}\>

Defined in: [src/airs/types.ts:996](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L996)

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

Defined in: [src/airs/types.ts:1008](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L1008)

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

Defined in: [src/airs/types.ts:969](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L969)

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<[`ModelSecurityGroup`](ModelSecurityGroup.md)\>

***

### getLabelKeys()

> **getLabelKeys**(`opts?`): `Promise`\<\{ `keys`: `string`[]; `totalItems`: `number`; \}\>

Defined in: [src/airs/types.ts:1016](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L1016)

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

Defined in: [src/airs/types.ts:1020](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L1020)

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

Defined in: [src/airs/types.ts:1030](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L1030)

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<`ModelSecurityModel`\>

***

### getModelVersion()

> **getModelVersion**(`uuid`): `Promise`\<`ModelSecurityModelVersion`\>

Defined in: [src/airs/types.ts:1035](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L1035)

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<`ModelSecurityModelVersion`\>

***

### getPyPIAuth()

> **getPyPIAuth**(): `Promise`\<[`ModelSecurityPyPIAuth`](ModelSecurityPyPIAuth.md)\>

Defined in: [src/airs/types.ts:1025](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L1025)

#### Returns

`Promise`\<[`ModelSecurityPyPIAuth`](ModelSecurityPyPIAuth.md)\>

***

### getRule()

> **getRule**(`uuid`): `Promise`\<[`ModelSecurityRule`](ModelSecurityRule.md)\>

Defined in: [src/airs/types.ts:988](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L988)

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<[`ModelSecurityRule`](ModelSecurityRule.md)\>

***

### getRuleInstance()

> **getRuleInstance**(`groupUuid`, `instanceUuid`): `Promise`\<[`ModelSecurityRuleInstance`](ModelSecurityRuleInstance.md)\>

Defined in: [src/airs/types.ts:978](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L978)

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

Defined in: [src/airs/types.ts:994](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L994)

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<[`ModelSecurityScan`](ModelSecurityScan.md)\>

***

### getViolation()

> **getViolation**(`uuid`): `Promise`\<[`ModelSecurityViolation`](ModelSecurityViolation.md)\>

Defined in: [src/airs/types.ts:1006](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L1006)

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<[`ModelSecurityViolation`](ModelSecurityViolation.md)\>

***

### getViolations()

> **getViolations**(`scanUuid`, `opts?`): `Promise`\<\{ `totalItems`: `number`; `violations`: [`ModelSecurityViolation`](ModelSecurityViolation.md)[]; \}\>

Defined in: [src/airs/types.ts:1002](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L1002)

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

Defined in: [src/airs/types.ts:966](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L966)

#### Parameters

##### opts?

[`ModelSecurityGroupListOptions`](ModelSecurityGroupListOptions.md)

#### Returns

`Promise`\<\{ `groups`: [`ModelSecurityGroup`](ModelSecurityGroup.md)[]; `totalItems`: `number`; \}\>

***

### listModels()

> **listModels**(`opts?`): `Promise`\<\{ `models`: `ModelSecurityModel`[]; `totalItems`: `number`; \}\>

Defined in: [src/airs/types.ts:1027](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L1027)

#### Parameters

##### opts?

`ModelSecurityModelListOptions`

#### Returns

`Promise`\<\{ `models`: `ModelSecurityModel`[]; `totalItems`: `number`; \}\>

***

### listModelVersionFiles()

> **listModelVersionFiles**(`modelVersionUuid`, `opts?`): `Promise`\<\{ `files`: [`ModelSecurityFile`](ModelSecurityFile.md)[]; `totalItems`: `number`; \}\>

Defined in: [src/airs/types.ts:1036](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L1036)

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

Defined in: [src/airs/types.ts:1031](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L1031)

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

Defined in: [src/airs/types.ts:974](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L974)

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

Defined in: [src/airs/types.ts:985](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L985)

#### Parameters

##### opts?

[`ModelSecurityRuleListOptions`](ModelSecurityRuleListOptions.md)

#### Returns

`Promise`\<\{ `rules`: [`ModelSecurityRule`](ModelSecurityRule.md)[]; `totalItems`: `number`; \}\>

***

### listScans()

> **listScans**(`opts?`): `Promise`\<\{ `scans`: [`ModelSecurityScan`](ModelSecurityScan.md)[]; `totalItems`: `number`; \}\>

Defined in: [src/airs/types.ts:991](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L991)

#### Parameters

##### opts?

[`ModelSecurityScanListOptions`](ModelSecurityScanListOptions.md)

#### Returns

`Promise`\<\{ `scans`: [`ModelSecurityScan`](ModelSecurityScan.md)[]; `totalItems`: `number`; \}\>

***

### setLabels()

> **setLabels**(`scanUuid`, `labels`): `Promise`\<`void`\>

Defined in: [src/airs/types.ts:1014](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L1014)

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

Defined in: [src/airs/types.ts:971](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L971)

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

Defined in: [src/airs/types.ts:979](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L979)

#### Parameters

##### groupUuid

`string`

##### instanceUuid

`string`

##### request

[`ModelSecurityRuleInstanceUpdateRequest`](ModelSecurityRuleInstanceUpdateRequest.md)

#### Returns

`Promise`\<[`ModelSecurityRuleInstance`](ModelSecurityRuleInstance.md)\>
