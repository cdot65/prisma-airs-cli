# Interface: RuntimeScanResult

Defined in: src/airs/types.ts:49

Normalized result from a runtime prompt scan (sync or async).

## Properties

### action

> **action**: `"allow"` \| `"block"`

Defined in: src/airs/types.ts:54

***

### category

> **category**: `string`

Defined in: src/airs/types.ts:55

***

### detections

> **detections**: `Record`\<`string`, `boolean`\>

Defined in: src/airs/types.ts:57

***

### error?

> `optional` **error?**: `string`

Defined in: src/airs/types.ts:58

***

### prompt

> **prompt**: `string`

Defined in: src/airs/types.ts:50

***

### reportId

> **reportId**: `string`

Defined in: src/airs/types.ts:53

***

### response?

> `optional` **response?**: `string`

Defined in: src/airs/types.ts:51

***

### scanId

> **scanId**: `string`

Defined in: src/airs/types.ts:52

***

### triggered

> **triggered**: `boolean`

Defined in: src/airs/types.ts:56
