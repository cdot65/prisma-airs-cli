# Class: SdkRuntimeService

Defined in: [src/airs/runtime.ts:26](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/runtime.ts#L26)

Contract for runtime scanning operations (sync + async).

## Implements

- [`RuntimeService`](../interfaces/RuntimeService.md)

## Constructors

### Constructor

> **new SdkRuntimeService**(`apiKey`): `SdkRuntimeService`

Defined in: [src/airs/runtime.ts:29](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/runtime.ts#L29)

#### Parameters

##### apiKey

`string`

#### Returns

`SdkRuntimeService`

## Methods

### pollResults()

> **pollResults**(`scanIds`, `intervalMs?`, `retryOpts?`): `Promise`\<[`RuntimeScanResult`](../interfaces/RuntimeScanResult.md)[]\>

Defined in: [src/airs/runtime.ts:103](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/runtime.ts#L103)

Poll async scan results until all complete or fail.

Note: The async query API (`queryByScanIds`) does not return `prompt`,
`response`, `triggered`, or `detections` fields. These are set to
defaults (`''`, `undefined`, `false`, `{}`) in the returned results.
Use `scanPrompt()` (sync API) when these fields are needed.

#### Parameters

##### scanIds

`string`[]

##### intervalMs?

`number` = `DEFAULT_POLL_INTERVAL_MS`

##### retryOpts?

`PollRetryOptions`

#### Returns

`Promise`\<[`RuntimeScanResult`](../interfaces/RuntimeScanResult.md)[]\>

#### Implementation of

[`RuntimeService`](../interfaces/RuntimeService.md).[`pollResults`](../interfaces/RuntimeService.md#pollresults)

***

### scanPrompt()

> **scanPrompt**(`profileName`, `prompt`, `response?`): `Promise`\<[`RuntimeScanResult`](../interfaces/RuntimeScanResult.md)\>

Defined in: [src/airs/runtime.ts:34](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/runtime.ts#L34)

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

[`RuntimeService`](../interfaces/RuntimeService.md).[`scanPrompt`](../interfaces/RuntimeService.md#scanprompt)

***

### submitBulkScan()

> **submitBulkScan**(`profileName`, `prompts`, `sessionId?`): `Promise`\<`string`[]\>

Defined in: [src/airs/runtime.ts:70](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/runtime.ts#L70)

Submit prompts for async bulk scanning, returns scan IDs.

#### Parameters

##### profileName

`string`

##### prompts

`string`[]

##### sessionId?

`string`

#### Returns

`Promise`\<`string`[]\>

#### Implementation of

[`RuntimeService`](../interfaces/RuntimeService.md).[`submitBulkScan`](../interfaces/RuntimeService.md#submitbulkscan)

***

### formatResultsCsv()

> `static` **formatResultsCsv**(`results`): `string`

Defined in: [src/airs/runtime.ts:198](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/runtime.ts#L198)

#### Parameters

##### results

[`RuntimeScanResult`](../interfaces/RuntimeScanResult.md)[]

#### Returns

`string`
