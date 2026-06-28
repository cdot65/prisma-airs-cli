# Interface: RuntimeScanResult

Defined in: [src/airs/types.ts:45](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L45)

Normalized result from a runtime prompt scan (sync or async).

## Properties

### action

> **action**: `"allow"` \| `"block"`

Defined in: [src/airs/types.ts:50](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L50)

***

### category

> **category**: `string`

Defined in: [src/airs/types.ts:51](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L51)

***

### detections

> **detections**: `Record`\<`string`, `boolean`\>

Defined in: [src/airs/types.ts:53](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L53)

***

### prompt

> **prompt**: `string`

Defined in: [src/airs/types.ts:46](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L46)

***

### reportId

> **reportId**: `string`

Defined in: [src/airs/types.ts:49](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L49)

***

### response?

> `optional` **response?**: `string`

Defined in: [src/airs/types.ts:47](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L47)

***

### scanId

> **scanId**: `string`

Defined in: [src/airs/types.ts:48](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L48)

***

### triggered

> **triggered**: `boolean`

Defined in: [src/airs/types.ts:52](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L52)
