# Interface: ScanResult

Defined in: prisma-airs-cli/src/airs/types.ts:33

Normalized output from a single AIRS prompt scan.

## Properties

### action

> **action**: `"allow"` \| `"block"`

Defined in: prisma-airs-cli/src/airs/types.ts:36

***

### category?

> `optional` **category?**: `string`

Defined in: prisma-airs-cli/src/airs/types.ts:39

***

### raw?

> `optional` **raw?**: `unknown`

Defined in: prisma-airs-cli/src/airs/types.ts:40

***

### reportId

> **reportId**: `string`

Defined in: prisma-airs-cli/src/airs/types.ts:35

***

### scanId

> **scanId**: `string`

Defined in: prisma-airs-cli/src/airs/types.ts:34

***

### triggered

> **triggered**: `boolean`

Defined in: prisma-airs-cli/src/airs/types.ts:38

Whether the topic guardrail was triggered for this prompt.
