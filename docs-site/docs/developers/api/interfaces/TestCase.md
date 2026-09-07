# Interface: TestCase

Defined in: prisma-airs-cli/src/core/types.ts:38

## Properties

### category

> **category**: `string`

Defined in: prisma-airs-cli/src/core/types.ts:41

***

### expectedTriggered

> **expectedTriggered**: `boolean`

Defined in: prisma-airs-cli/src/core/types.ts:40

***

### prompt

> **prompt**: `string`

Defined in: prisma-airs-cli/src/core/types.ts:39

***

### source?

> `optional` **source?**: `"generated"` \| `"carried-fp"` \| `"carried-fn"` \| `"regression"`

Defined in: prisma-airs-cli/src/core/types.ts:43

How this test entered the suite. Default: 'generated'.

***

### targetTopic?

> `optional` **targetTopic?**: `string`

Defined in: prisma-airs-cli/src/core/types.ts:45

Which topic this test targets (used by audit).
