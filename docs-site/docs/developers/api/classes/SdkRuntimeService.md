# Class: SdkRuntimeService

Defined in: [src/airs/runtime.ts:203](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/runtime.ts#L203)

Runtime scanning contract with item-correlated, resumable bulk operations.

## Implements

- [`ReliableRuntimeService`](../interfaces/ReliableRuntimeService.md)

## Constructors

### Constructor

> **new SdkRuntimeService**(`opts`): `SdkRuntimeService`

Defined in: [src/airs/runtime.ts:206](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/runtime.ts#L206)

#### Parameters

##### opts

`InitOptions`

#### Returns

`SdkRuntimeService`

## Methods

### pollBatch()

> **pollBatch**(`batch`, `intervalMs?`, `retryOpts?`): `Promise`\<[`BulkScanResult`](../interfaces/BulkScanResult.md)[]\>

Defined in: [src/airs/runtime.ts:291](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/runtime.ts#L291)

Poll one async submission and return one result per prompt, ordered by input index.

#### Parameters

##### batch

[`SubmittedBatch`](../interfaces/SubmittedBatch.md)

##### intervalMs?

`number` = `DEFAULT_POLL_INTERVAL_MS`

##### retryOpts?

[`PollRetryOptions`](../interfaces/PollRetryOptions.md)

#### Returns

`Promise`\<[`BulkScanResult`](../interfaces/BulkScanResult.md)[]\>

#### Implementation of

[`ReliableRuntimeService`](../interfaces/ReliableRuntimeService.md).[`pollBatch`](../interfaces/ReliableRuntimeService.md#pollbatch)

***

### ~~pollResults()~~

> **pollResults**(`scanIds`, `intervalMs?`, `retryOpts?`): `Promise`\<[`RuntimeScanResult`](../interfaces/RuntimeScanResult.md)[]\>

Defined in: [src/airs/runtime.ts:500](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/runtime.ts#L500)

Compatibility poller for callers that retained only batch scan IDs.
Nested detection data is preserved, but prompt text and per-request fan-out
cannot be reconstructed from scan IDs alone.

#### Parameters

##### scanIds

`string`[]

##### intervalMs?

`number` = `DEFAULT_POLL_INTERVAL_MS`

##### retryOpts?

[`PollRetryOptions`](../interfaces/PollRetryOptions.md)

#### Returns

`Promise`\<[`RuntimeScanResult`](../interfaces/RuntimeScanResult.md)[]\>

#### Deprecated

Use pollBatch to preserve `(scan_id, req_id)` correlation and prompt text.

#### Implementation of

[`ReliableRuntimeService`](../interfaces/ReliableRuntimeService.md).[`pollResults`](../interfaces/ReliableRuntimeService.md#pollresults)

***

### scanPrompt()

> **scanPrompt**(`profileName`, `prompt`, `response?`): `Promise`\<[`RuntimeScanResult`](../interfaces/RuntimeScanResult.md)\>

Defined in: [src/airs/runtime.ts:211](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/runtime.ts#L211)

Scan a single prompt (and optional response) synchronously.

#### Parameters

##### profileName

`string`

##### prompt

`string`

##### response?

`string`

#### Returns

`Promise`\<[`RuntimeScanResult`](../interfaces/RuntimeScanResult.md)\>

#### Implementation of

[`ReliableRuntimeService`](../interfaces/ReliableRuntimeService.md).[`scanPrompt`](../interfaces/ReliableRuntimeService.md#scanprompt)

***

### submitBatch()

> **submitBatch**(`profileName`, `prompts`, `sessionId?`, `retryOpts?`): `Promise`\<[`SubmittedBatch`](../interfaces/SubmittedBatch.md)\>

Defined in: [src/airs/runtime.ts:229](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/runtime.ts#L229)

Submit one SDK-sized group of indexed prompts for async scanning.

#### Parameters

##### profileName

`string`

##### prompts

[`IndexedPrompt`](../interfaces/IndexedPrompt.md)[]

##### sessionId?

`string`

##### retryOpts?

[`PollRetryOptions`](../interfaces/PollRetryOptions.md)

#### Returns

`Promise`\<[`SubmittedBatch`](../interfaces/SubmittedBatch.md)\>

#### Implementation of

[`ReliableRuntimeService`](../interfaces/ReliableRuntimeService.md).[`submitBatch`](../interfaces/ReliableRuntimeService.md#submitbatch)

***

### ~~submitBulkScan()~~

> **submitBulkScan**(`profileName`, `prompts`, `sessionId?`): `Promise`\<`string`[]\>

Defined in: [src/airs/runtime.ts:469](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/runtime.ts#L469)

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

Use submitBatch to preserve per-prompt request correlation.

#### Implementation of

[`ReliableRuntimeService`](../interfaces/ReliableRuntimeService.md).[`submitBulkScan`](../interfaces/ReliableRuntimeService.md#submitbulkscan)

***

### formatResultsCsv()

> `static` **formatResultsCsv**(`results`): `string`

Defined in: [src/airs/runtime.ts:597](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/runtime.ts#L597)

#### Parameters

##### results

([`RuntimeScanResult`](../interfaces/RuntimeScanResult.md) \| [`BulkScanResult`](../interfaces/BulkScanResult.md))[]

#### Returns

`string`
