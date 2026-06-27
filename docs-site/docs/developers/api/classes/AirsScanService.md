# Class: AirsScanService

Defined in: [src/airs/scanner.ts:7](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/scanner.ts#L7)

Scans prompts against AIRS security profiles via the Prisma AIRS SDK.

## Implements

- `ScanService`

## Constructors

### Constructor

> **new AirsScanService**(`apiKey`): `AirsScanService`

Defined in: [src/airs/scanner.ts:10](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/scanner.ts#L10)

#### Parameters

##### apiKey

`string`

#### Returns

`AirsScanService`

## Methods

### scan()

> **scan**(`profileName`, `prompt`, `sessionId?`): `Promise`\<`ScanResult`\>

Defined in: [src/airs/scanner.ts:16](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/scanner.ts#L16)

Scan a single prompt synchronously and return the normalized result.

#### Parameters

##### profileName

`string`

##### prompt

`string`

##### sessionId?

`string`

#### Returns

`Promise`\<`ScanResult`\>

#### Implementation of

`ScanService.scan`

***

### scanBatch()

> **scanBatch**(`profileName`, `prompts`, `concurrency?`, `sessionId?`): `Promise`\<`ScanResult`[]\>

Defined in: [src/airs/scanner.ts:40](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/scanner.ts#L40)

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

`Promise`\<`ScanResult`[]\>

#### Implementation of

`ScanService.scanBatch`
