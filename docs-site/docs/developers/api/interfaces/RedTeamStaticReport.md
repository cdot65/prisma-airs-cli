# Interface: RedTeamStaticReport

Defined in: [src/airs/types.ts:266](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L266)

Normalized static report summary.

## Properties

### asr?

> `optional` **asr?**: `number` \| `null`

Defined in: [src/airs/types.ts:268](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L268)

***

### categories

> **categories**: `object`[]

Defined in: [src/airs/types.ts:275](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L275)

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

Defined in: [src/airs/types.ts:274](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L274)

***

### score?

> `optional` **score?**: `number` \| `null`

Defined in: [src/airs/types.ts:267](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L267)

***

### severityBreakdown

> **severityBreakdown**: `object`[]

Defined in: [src/airs/types.ts:269](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L269)

#### failed

> **failed**: `number`

#### severity

> **severity**: `string`

#### successful

> **successful**: `number`
