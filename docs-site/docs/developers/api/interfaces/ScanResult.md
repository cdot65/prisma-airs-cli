# Interface: ScanResult

Defined in: src/airs/types.ts:34

Normalized output from a single AIRS prompt scan.

## Properties

### action

> **action**: `"allow"` \| `"block"`

Defined in: src/airs/types.ts:37

***

### category?

> `optional` **category?**: `string`

Defined in: src/airs/types.ts:40

***

### raw?

> `optional` **raw?**: `unknown`

Defined in: src/airs/types.ts:41

***

### reportId

> **reportId**: `string`

Defined in: src/airs/types.ts:36

***

### scanId

> **scanId**: `string`

Defined in: src/airs/types.ts:35

***

### triggered

> **triggered**: `boolean`

Defined in: src/airs/types.ts:39

Whether the topic guardrail was triggered for this prompt.
