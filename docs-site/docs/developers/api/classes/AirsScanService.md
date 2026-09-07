# Class: AirsScanService

Defined in: src/airs/scanner.ts:7

Scans prompts against AIRS security profiles via the Prisma AIRS SDK.

## Implements

- [`ScanService`](../interfaces/ScanService.md)

## Constructors

### Constructor

> **new AirsScanService**(`opts`): `AirsScanService`

Defined in: src/airs/scanner.ts:10

#### Parameters

##### opts

`InitOptions`

#### Returns

`AirsScanService`

## Methods

### scan()

> **scan**(`profileName`, `prompt`, `sessionId?`): `Promise`\<[`ScanResult`](../interfaces/ScanResult.md)\>

Defined in: src/airs/scanner.ts:16

Scan a single prompt synchronously and return the normalized result.

#### Parameters

##### profileName

`string`

##### prompt

`string`

##### sessionId?

`string`

#### Returns

`Promise`\<[`ScanResult`](../interfaces/ScanResult.md)\>

#### Implementation of

[`ScanService`](../interfaces/ScanService.md).[`scan`](../interfaces/ScanService.md#scan)

***

### scanBatch()

> **scanBatch**(`profileName`, `prompts`, `concurrency?`, `sessionId?`): `Promise`\<[`ScanResult`](../interfaces/ScanResult.md)[]\>

Defined in: src/airs/scanner.ts:40

Scan multiple prompts concurrently (default 5) and return results in order.

#### Parameters

##### profileName

`string`

##### prompts

`string`[]

##### concurrency?

`number` = `5`

##### sessionId?

`string`

#### Returns

`Promise`\<[`ScanResult`](../interfaces/ScanResult.md)[]\>

#### Implementation of

[`ScanService`](../interfaces/ScanService.md).[`scanBatch`](../interfaces/ScanService.md#scanbatch)
