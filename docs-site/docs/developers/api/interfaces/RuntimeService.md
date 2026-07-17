# Interface: RuntimeService

Defined in: [src/airs/types.ts:87](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L87)

Backwards-compatible contract for the original runtime scanning operations.

## Extended by

- [`ReliableRuntimeService`](ReliableRuntimeService.md)

## Methods

### ~~pollResults()~~

> **pollResults**(`scanIds`, `intervalMs?`): `Promise`\<[`RuntimeScanResult`](RuntimeScanResult.md)[]\>

Defined in: [src/airs/types.ts:93](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L93)

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

Defined in: [src/airs/types.ts:89](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L89)

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

Defined in: [src/airs/types.ts:91](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L91)

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
