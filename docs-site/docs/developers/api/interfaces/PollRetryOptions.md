# Interface: PollRetryOptions

Defined in: src/airs/runtime.ts:55

## Properties

### baseDelayMs?

> `optional` **baseDelayMs?**: `number`

Defined in: src/airs/runtime.ts:59

Base delay in ms for exponential backoff. Default: 10000.

***

### maxNoProgressPolls?

> `optional` **maxNoProgressPolls?**: `number`

Defined in: src/airs/runtime.ts:61

Stop after this many consecutive successful polls resolve no new prompts. Default: 120.

***

### maxRetries?

> `optional` **maxRetries?**: `number`

Defined in: src/airs/runtime.ts:57

Max retries per rate-limit error before giving up. Default: 5.

***

### onProgress?

> `optional` **onProgress?**: (`results`) => `void` \| `Promise`\<`void`\>

Defined in: src/airs/runtime.ts:65

Called whenever newly terminal prompt results are resolved.

#### Parameters

##### results

[`BulkScanResult`](BulkScanResult.md)[]

#### Returns

`void` \| `Promise`\<`void`\>

***

### onRetry?

> `optional` **onRetry?**: (`attempt`, `delayMs`) => `void`

Defined in: src/airs/runtime.ts:63

Called on each retry with (attempt, delayMs).

#### Parameters

##### attempt

`number`

##### delayMs

`number`

#### Returns

`void`
