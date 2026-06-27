# Interface: ModelSecurityService

Defined in: [src/airs/types.ts:720](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L720)

Contract for Model Security operations.

## Methods

### addLabels()

> **addLabels**(`scanUuid`, `labels`): `Promise`\<`void`\>

Defined in: [src/airs/types.ts:768](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L768)

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

Defined in: [src/airs/types.ts:725](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L725)

#### Parameters

##### request

[`ModelSecurityGroupCreateRequest`](ModelSecurityGroupCreateRequest.md)

#### Returns

`Promise`\<[`ModelSecurityGroup`](ModelSecurityGroup.md)\>

***

### createScan()

> **createScan**(`request`): `Promise`\<[`ModelSecurityScan`](ModelSecurityScan.md)\>

Defined in: [src/airs/types.ts:745](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L745)

#### Parameters

##### request

`Record`\<`string`, `unknown`\>

#### Returns

`Promise`\<[`ModelSecurityScan`](ModelSecurityScan.md)\>

***

### deleteGroup()

> **deleteGroup**(`uuid`): `Promise`\<`void`\>

Defined in: [src/airs/types.ts:727](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L727)

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<`void`\>

***

### deleteLabels()

> **deleteLabels**(`scanUuid`, `keys`): `Promise`\<`void`\>

Defined in: [src/airs/types.ts:770](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L770)

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

Defined in: [src/airs/types.ts:755](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L755)

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<[`ModelSecurityEvaluation`](ModelSecurityEvaluation.md)\>

***

### getEvaluations()

> **getEvaluations**(`scanUuid`, `opts?`): `Promise`\<\{ `evaluations`: [`ModelSecurityEvaluation`](ModelSecurityEvaluation.md)[]; `totalItems`: `number`; \}\>

Defined in: [src/airs/types.ts:751](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L751)

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

Defined in: [src/airs/types.ts:763](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L763)

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

Defined in: [src/airs/types.ts:724](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L724)

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<[`ModelSecurityGroup`](ModelSecurityGroup.md)\>

***

### getLabelKeys()

> **getLabelKeys**(`opts?`): `Promise`\<\{ `keys`: `string`[]; `totalItems`: `number`; \}\>

Defined in: [src/airs/types.ts:771](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L771)

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

Defined in: [src/airs/types.ts:775](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L775)

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

### getPyPIAuth()

> **getPyPIAuth**(): `Promise`\<[`ModelSecurityPyPIAuth`](ModelSecurityPyPIAuth.md)\>

Defined in: [src/airs/types.ts:780](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L780)

#### Returns

`Promise`\<[`ModelSecurityPyPIAuth`](ModelSecurityPyPIAuth.md)\>

***

### getRule()

> **getRule**(`uuid`): `Promise`\<[`ModelSecurityRule`](ModelSecurityRule.md)\>

Defined in: [src/airs/types.ts:743](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L743)

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<[`ModelSecurityRule`](ModelSecurityRule.md)\>

***

### getRuleInstance()

> **getRuleInstance**(`groupUuid`, `instanceUuid`): `Promise`\<[`ModelSecurityRuleInstance`](ModelSecurityRuleInstance.md)\>

Defined in: [src/airs/types.ts:733](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L733)

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

Defined in: [src/airs/types.ts:749](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L749)

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<[`ModelSecurityScan`](ModelSecurityScan.md)\>

***

### getViolation()

> **getViolation**(`uuid`): `Promise`\<[`ModelSecurityViolation`](ModelSecurityViolation.md)\>

Defined in: [src/airs/types.ts:761](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L761)

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<[`ModelSecurityViolation`](ModelSecurityViolation.md)\>

***

### getViolations()

> **getViolations**(`scanUuid`, `opts?`): `Promise`\<\{ `totalItems`: `number`; `violations`: [`ModelSecurityViolation`](ModelSecurityViolation.md)[]; \}\>

Defined in: [src/airs/types.ts:757](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L757)

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

Defined in: [src/airs/types.ts:721](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L721)

#### Parameters

##### opts?

[`ModelSecurityGroupListOptions`](ModelSecurityGroupListOptions.md)

#### Returns

`Promise`\<\{ `groups`: [`ModelSecurityGroup`](ModelSecurityGroup.md)[]; `totalItems`: `number`; \}\>

***

### listRuleInstances()

> **listRuleInstances**(`groupUuid`, `opts?`): `Promise`\<\{ `ruleInstances`: [`ModelSecurityRuleInstance`](ModelSecurityRuleInstance.md)[]; `totalItems`: `number`; \}\>

Defined in: [src/airs/types.ts:729](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L729)

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

Defined in: [src/airs/types.ts:740](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L740)

#### Parameters

##### opts?

[`ModelSecurityRuleListOptions`](ModelSecurityRuleListOptions.md)

#### Returns

`Promise`\<\{ `rules`: [`ModelSecurityRule`](ModelSecurityRule.md)[]; `totalItems`: `number`; \}\>

***

### listScans()

> **listScans**(`opts?`): `Promise`\<\{ `scans`: [`ModelSecurityScan`](ModelSecurityScan.md)[]; `totalItems`: `number`; \}\>

Defined in: [src/airs/types.ts:746](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L746)

#### Parameters

##### opts?

[`ModelSecurityScanListOptions`](ModelSecurityScanListOptions.md)

#### Returns

`Promise`\<\{ `scans`: [`ModelSecurityScan`](ModelSecurityScan.md)[]; `totalItems`: `number`; \}\>

***

### setLabels()

> **setLabels**(`scanUuid`, `labels`): `Promise`\<`void`\>

Defined in: [src/airs/types.ts:769](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L769)

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

Defined in: [src/airs/types.ts:726](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L726)

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

Defined in: [src/airs/types.ts:734](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L734)

#### Parameters

##### groupUuid

`string`

##### instanceUuid

`string`

##### request

[`ModelSecurityRuleInstanceUpdateRequest`](ModelSecurityRuleInstanceUpdateRequest.md)

#### Returns

`Promise`\<[`ModelSecurityRuleInstance`](ModelSecurityRuleInstance.md)\>
