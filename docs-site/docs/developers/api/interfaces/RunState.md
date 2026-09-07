# Interface: RunState

Defined in: src/core/types.ts:104

## Properties

### bestCoverage

> **bestCoverage**: `number`

Defined in: src/core/types.ts:112

***

### bestIteration

> **bestIteration**: `number`

Defined in: src/core/types.ts:111

***

### companionTopic?

> `optional` **companionTopic?**: [`CustomTopic`](CustomTopic.md)

Defined in: src/core/types.ts:117

Companion allow topic created for block-intent two-phase generation.

***

### consecutiveRegressions

> **consecutiveRegressions**: `number`

Defined in: src/core/types.ts:113

***

### createdAt

> **createdAt**: `string`

Defined in: src/core/types.ts:106

***

### currentIteration

> **currentIteration**: `number`

Defined in: src/core/types.ts:110

***

### hasRevertedToBest

> **hasRevertedToBest**: `boolean`

Defined in: src/core/types.ts:114

***

### hasTriedSimplification

> **hasTriedSimplification**: `boolean`

Defined in: src/core/types.ts:115

***

### id

> **id**: `string`

Defined in: src/core/types.ts:105

***

### iterations

> **iterations**: [`IterationResult`](IterationResult.md)[]

Defined in: src/core/types.ts:109

***

### status

> **status**: `"failed"` \| `"completed"` \| `"running"` \| `"paused"`

Defined in: src/core/types.ts:118

***

### updatedAt

> **updatedAt**: `string`

Defined in: src/core/types.ts:107

***

### userInput

> **userInput**: [`UserInput`](UserInput.md)

Defined in: src/core/types.ts:108
