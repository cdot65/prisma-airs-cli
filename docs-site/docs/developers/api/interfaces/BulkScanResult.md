# Interface: BulkScanResult

Defined in: src/airs/types.ts:80

A normalized async result with its stable input position and AIRS request ID.

## Extends

- `Omit`\<[`RuntimeScanResult`](RuntimeScanResult.md), `"action"`\>

## Properties

### action

> **action**: [`BulkScanAction`](../type-aliases/BulkScanAction.md)

Defined in: src/airs/types.ts:83

***

### category

> **category**: `string`

Defined in: src/airs/types.ts:51

#### Inherited from

[`RuntimeScanResult`](RuntimeScanResult.md).[`category`](RuntimeScanResult.md#category)

***

### detections

> **detections**: `Record`\<`string`, `boolean`\>

Defined in: src/airs/types.ts:53

#### Inherited from

[`RuntimeScanResult`](RuntimeScanResult.md).[`detections`](RuntimeScanResult.md#detections)

***

### error?

> `optional` **error?**: `string`

Defined in: src/airs/types.ts:54

#### Inherited from

[`RuntimeScanResult`](RuntimeScanResult.md).[`error`](RuntimeScanResult.md#error)

***

### index

> **index**: `number`

Defined in: src/airs/types.ts:81

***

### prompt

> **prompt**: `string`

Defined in: src/airs/types.ts:46

#### Inherited from

[`RuntimeScanResult`](RuntimeScanResult.md).[`prompt`](RuntimeScanResult.md#prompt)

***

### reportId

> **reportId**: `string`

Defined in: src/airs/types.ts:49

#### Inherited from

[`RuntimeScanResult`](RuntimeScanResult.md).[`reportId`](RuntimeScanResult.md#reportid)

***

### reqId

> **reqId**: `number`

Defined in: src/airs/types.ts:82

***

### response?

> `optional` **response?**: `string`

Defined in: src/airs/types.ts:47

#### Inherited from

[`RuntimeScanResult`](RuntimeScanResult.md).[`response`](RuntimeScanResult.md#response)

***

### scanId

> **scanId**: `string`

Defined in: src/airs/types.ts:48

#### Inherited from

[`RuntimeScanResult`](RuntimeScanResult.md).[`scanId`](RuntimeScanResult.md#scanid)

***

### triggered

> **triggered**: `boolean`

Defined in: src/airs/types.ts:52

#### Inherited from

[`RuntimeScanResult`](RuntimeScanResult.md).[`triggered`](RuntimeScanResult.md#triggered)
