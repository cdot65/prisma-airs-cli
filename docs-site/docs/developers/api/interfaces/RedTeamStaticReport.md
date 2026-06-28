# Interface: RedTeamStaticReport

Defined in: [src/airs/types.ts:274](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L274)

Normalized static report summary.

## Properties

### asr?

> `optional` **asr?**: `number` \| `null`

Defined in: [src/airs/types.ts:276](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L276)

***

### categories

> **categories**: `object`[]

Defined in: [src/airs/types.ts:283](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L283)

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

Defined in: [src/airs/types.ts:282](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L282)

***

### score?

> `optional` **score?**: `number` \| `null`

Defined in: [src/airs/types.ts:275](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L275)

***

### severityBreakdown

> **severityBreakdown**: `object`[]

Defined in: [src/airs/types.ts:277](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L277)

#### failed

> **failed**: `number`

#### severity

> **severity**: `string`

#### successful

> **successful**: `number`
