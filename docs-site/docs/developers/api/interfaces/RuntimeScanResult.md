# Interface: RuntimeScanResult

Defined in: src/airs/types.ts:45

Normalized result from a runtime prompt scan (sync or async).

## Properties

### action

> **action**: `"allow"` \| `"block"`

Defined in: src/airs/types.ts:50

***

### category

> **category**: `string`

Defined in: src/airs/types.ts:51

***

### detections

> **detections**: `Record`\<`string`, `boolean`\>

Defined in: src/airs/types.ts:53

***

### error?

> `optional` **error?**: `string`

Defined in: src/airs/types.ts:54

***

### prompt

> **prompt**: `string`

Defined in: src/airs/types.ts:46

***

### reportId

> **reportId**: `string`

Defined in: src/airs/types.ts:49

***

### response?

> `optional` **response?**: `string`

Defined in: src/airs/types.ts:47

***

### scanId

> **scanId**: `string`

Defined in: src/airs/types.ts:48

***

### triggered

> **triggered**: `boolean`

Defined in: src/airs/types.ts:52
