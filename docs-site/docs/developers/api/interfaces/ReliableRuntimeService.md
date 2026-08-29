# Interface: ReliableRuntimeService

Defined in: src/airs/types.ts:97

Runtime scanning contract with item-correlated, resumable bulk operations.

## Extends

- [`RuntimeService`](RuntimeService.md)

## Methods

### pollBatch()

> **pollBatch**(`batch`, `intervalMs?`, `retryOpts?`): `Promise`\<[`BulkScanResult`](BulkScanResult.md)[]\>

Defined in: src/airs/types.ts:112

Poll one async submission and return one result per prompt, ordered by input index.

#### Parameters

##### batch

[`SubmittedBatch`](SubmittedBatch.md)

##### intervalMs?

`number`

##### retryOpts?

###### baseDelayMs?

`number`

###### maxNoProgressPolls?

`number`

###### maxRetries?

`number`

###### onProgress?

(`results`) => `void` \| `Promise`\<`void`\>

###### onRetry?

(`attempt`, `delayMs`) => `void`

#### Returns

`Promise`\<[`BulkScanResult`](BulkScanResult.md)[]\>

***

### ~~pollResults()~~

> **pollResults**(`scanIds`, `intervalMs?`): `Promise`\<[`RuntimeScanResult`](RuntimeScanResult.md)[]\>

Defined in: src/airs/types.ts:93

#### Parameters

##### scanIds

`string`[]

##### intervalMs?

`number`

#### Returns

`Promise`\<[`RuntimeScanResult`](RuntimeScanResult.md)[]\>

#### Deprecated

Use ReliableRuntimeService.pollBatch to preserve per-prompt correlation.

#### Inherited from

[`RuntimeService`](RuntimeService.md).[`pollResults`](RuntimeService.md#pollresults)

***

### scanPrompt()

> **scanPrompt**(`profileName`, `prompt`, `response?`): `Promise`\<[`RuntimeScanResult`](RuntimeScanResult.md)\>

Defined in: src/airs/types.ts:89

Scan a single prompt (and optional response) synchronously.

#### Parameters

##### profileName

`string`

##### prompt

`string`

##### response?

`string`

#### Returns

`Promise`\<[`RuntimeScanResult`](RuntimeScanResult.md)\>

#### Inherited from

[`RuntimeService`](RuntimeService.md).[`scanPrompt`](RuntimeService.md#scanprompt)

***

### submitBatch()

> **submitBatch**(`profileName`, `prompts`, `sessionId?`, `retryOpts?`): `Promise`\<[`SubmittedBatch`](SubmittedBatch.md)\>

Defined in: src/airs/types.ts:99

Submit one SDK-sized group of indexed prompts for async scanning.

#### Parameters

##### profileName

`string`

##### prompts

[`IndexedPrompt`](IndexedPrompt.md)[]

##### sessionId?

`string`

##### retryOpts?

###### baseDelayMs?

`number`

###### maxNoProgressPolls?

`number`

###### maxRetries?

`number`

###### onProgress?

(`results`) => `void` \| `Promise`\<`void`\>

###### onRetry?

(`attempt`, `delayMs`) => `void`

#### Returns

`Promise`\<[`SubmittedBatch`](SubmittedBatch.md)\>

***

### ~~submitBulkScan()~~

> **submitBulkScan**(`profileName`, `prompts`, `sessionId?`): `Promise`\<`string`[]\>

Defined in: src/airs/types.ts:91

#### Parameters

##### profileName

`string`

##### prompts

`string`[]

##### sessionId?

`string`

#### Returns

`Promise`\<`string`[]\>

#### Deprecated

Use ReliableRuntimeService.submitBatch to preserve per-prompt correlation.

#### Inherited from

[`RuntimeService`](RuntimeService.md).[`submitBulkScan`](RuntimeService.md#submitbulkscan)
