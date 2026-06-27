# Interface: RuntimeScanResult

Defined in: [src/airs/types.ts:37](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L37)

Normalized result from a runtime prompt scan (sync or async).

## Properties

### action

> **action**: `"allow"` \| `"block"`

Defined in: [src/airs/types.ts:42](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L42)

***

### category

> **category**: `string`

Defined in: [src/airs/types.ts:43](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L43)

***

### detections

> **detections**: `Record`\<`string`, `boolean`\>

Defined in: [src/airs/types.ts:45](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L45)

***

### prompt

> **prompt**: `string`

Defined in: [src/airs/types.ts:38](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L38)

***

### reportId

> **reportId**: `string`

Defined in: [src/airs/types.ts:41](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L41)

***

### response?

> `optional` **response?**: `string`

Defined in: [src/airs/types.ts:39](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L39)

***

### scanId

> **scanId**: `string`

Defined in: [src/airs/types.ts:40](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L40)

***

### triggered

> **triggered**: `boolean`

Defined in: [src/airs/types.ts:44](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L44)
