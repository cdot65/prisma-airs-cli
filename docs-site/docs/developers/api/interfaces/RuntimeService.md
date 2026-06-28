# Interface: RuntimeService

Defined in: [src/airs/types.ts:57](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L57)

Contract for runtime scanning operations (sync + async).

## Methods

### pollResults()

> **pollResults**(`scanIds`, `intervalMs?`): `Promise`\<[`RuntimeScanResult`](RuntimeScanResult.md)[]\>

Defined in: [src/airs/types.ts:63](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L63)

Poll async scan results until all complete.

#### Parameters

##### scanIds

`string`[]

##### intervalMs?

`number`

#### Returns

`Promise`\<[`RuntimeScanResult`](RuntimeScanResult.md)[]\>

***

### scanPrompt()

> **scanPrompt**(`profileName`, `prompt`, `response?`): `Promise`\<[`RuntimeScanResult`](RuntimeScanResult.md)\>

Defined in: [src/airs/types.ts:59](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L59)

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

### submitBulkScan()

> **submitBulkScan**(`profileName`, `prompts`): `Promise`\<`string`[]\>

Defined in: [src/airs/types.ts:61](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L61)

Submit prompts for async bulk scanning, returns scan IDs.

#### Parameters

##### profileName

`string`

##### prompts

`string`[]

#### Returns

`Promise`\<`string`[]\>
