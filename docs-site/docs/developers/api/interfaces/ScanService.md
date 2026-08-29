# Interface: ScanService

Defined in: src/airs/types.ts:130

Contract for AIRS prompt scanning operations.

## Methods

### scan()

> **scan**(`profileName`, `prompt`, `sessionId?`): `Promise`\<[`ScanResult`](ScanResult.md)\>

Defined in: src/airs/types.ts:132

Scan a single prompt against a security profile.

#### Parameters

##### profileName

`string`

##### prompt

`string`

##### sessionId?

`string`

#### Returns

`Promise`\<[`ScanResult`](ScanResult.md)\>

***

### scanBatch()

> **scanBatch**(`profileName`, `prompts`, `concurrency?`, `sessionId?`): `Promise`\<[`ScanResult`](ScanResult.md)[]\>

Defined in: src/airs/types.ts:134

Scan multiple prompts with concurrency control.

#### Parameters

##### profileName

`string`

##### prompts

`string`[]

##### concurrency?

`number`

##### sessionId?

`string`

#### Returns

`Promise`\<[`ScanResult`](ScanResult.md)[]\>
