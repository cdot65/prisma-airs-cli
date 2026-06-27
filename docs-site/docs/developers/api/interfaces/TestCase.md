# Interface: TestCase

Defined in: [src/core/types.ts:38](https://github.com/cdot65/prisma-airs-cli/blob/main/src/core/types.ts#L38)

## Properties

### category

> **category**: `string`

Defined in: [src/core/types.ts:41](https://github.com/cdot65/prisma-airs-cli/blob/main/src/core/types.ts#L41)

***

### expectedTriggered

> **expectedTriggered**: `boolean`

Defined in: [src/core/types.ts:40](https://github.com/cdot65/prisma-airs-cli/blob/main/src/core/types.ts#L40)

***

### prompt

> **prompt**: `string`

Defined in: [src/core/types.ts:39](https://github.com/cdot65/prisma-airs-cli/blob/main/src/core/types.ts#L39)

***

### source?

> `optional` **source?**: `"generated"` \| `"carried-fp"` \| `"carried-fn"` \| `"regression"`

Defined in: [src/core/types.ts:43](https://github.com/cdot65/prisma-airs-cli/blob/main/src/core/types.ts#L43)

How this test entered the suite. Default: 'generated'.

***

### targetTopic?

> `optional` **targetTopic?**: `string`

Defined in: [src/core/types.ts:45](https://github.com/cdot65/prisma-airs-cli/blob/main/src/core/types.ts#L45)

Which topic this test targets (used by audit).
