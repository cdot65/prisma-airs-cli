# Interface: RedTeamStaticReport

Defined in: [src/airs/types.ts:333](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L333)

Normalized static report summary.

## Properties

### asr?

> `optional` **asr?**: `number` \| `null`

Defined in: [src/airs/types.ts:335](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L335)

***

### categories

> **categories**: `object`[]

Defined in: [src/airs/types.ts:342](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L342)

#### asr

> **asr**: `number`

#### displayName

> **displayName**: `string`

#### failed

> **failed**: `number`

#### id

> **id**: `string`

#### successful

> **successful**: `number`

#### total

> **total**: `number`

***

### reportSummary?

> `optional` **reportSummary?**: `string` \| `null`

Defined in: [src/airs/types.ts:341](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L341)

***

### score?

> `optional` **score?**: `number` \| `null`

Defined in: [src/airs/types.ts:334](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L334)

***

### severityBreakdown

> **severityBreakdown**: `object`[]

Defined in: [src/airs/types.ts:336](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L336)

#### failed

> **failed**: `number`

#### severity

> **severity**: `string`

#### successful

> **successful**: `number`
