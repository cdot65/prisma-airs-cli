# Interface: RedTeamAdapterVar

Defined in: src/airs/types.ts:1457

An adapter configuration variable. Secrets are masked; key off `isRedacted`, not the value.

## Properties

### isRedacted?

> `optional` **isRedacted?**: `boolean`

Defined in: src/airs/types.ts:1461

***

### key

> **key**: `string`

Defined in: src/airs/types.ts:1458

***

### type

> **type**: `"VAR"` \| `"SECRET"`

Defined in: src/airs/types.ts:1460

***

### value?

> `optional` **value?**: `string` \| `null`

Defined in: src/airs/types.ts:1459
