# Interface: RuntimeService

Defined in: src/airs/types.ts:87

Backwards-compatible contract for the original runtime scanning operations.

## Extended by

- [`ReliableRuntimeService`](ReliableRuntimeService.md)

## Methods

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
