# Interface: ScanResult

Defined in: src/airs/types.ts:30

Normalized output from a single AIRS prompt scan.

## Properties

### action

> **action**: `"allow"` \| `"block"`

Defined in: src/airs/types.ts:33

***

### category?

> `optional` **category?**: `string`

Defined in: src/airs/types.ts:36

***

### raw?

> `optional` **raw?**: `unknown`

Defined in: src/airs/types.ts:37

***

### reportId

> **reportId**: `string`

Defined in: src/airs/types.ts:32

***

### scanId

> **scanId**: `string`

Defined in: src/airs/types.ts:31

***

### triggered

> **triggered**: `boolean`

Defined in: src/airs/types.ts:35

Whether the topic guardrail was triggered for this prompt.
