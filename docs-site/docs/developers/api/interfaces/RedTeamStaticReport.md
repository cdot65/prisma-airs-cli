# Interface: RedTeamStaticReport

Defined in: prisma-airs-cli/src/airs/types.ts:336

Normalized static report summary.

## Properties

### asr?

> `optional` **asr?**: `number` \| `null`

Defined in: prisma-airs-cli/src/airs/types.ts:338

***

### categories

> **categories**: `object`[]

Defined in: prisma-airs-cli/src/airs/types.ts:345

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

Defined in: prisma-airs-cli/src/airs/types.ts:344

***

### score?

> `optional` **score?**: `number` \| `null`

Defined in: prisma-airs-cli/src/airs/types.ts:337

***

### severityBreakdown

> **severityBreakdown**: `object`[]

Defined in: prisma-airs-cli/src/airs/types.ts:339

#### failed

> **failed**: `number`

#### severity

> **severity**: `string`

#### successful

> **successful**: `number`
