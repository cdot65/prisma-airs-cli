# Interface: RedTeamAdapterVar

Defined in: src/airs/types.ts:1461

An adapter configuration variable. Secrets are masked; key off `isRedacted`, not the value.

## Properties

### isRedacted?

> `optional` **isRedacted?**: `boolean`

Defined in: src/airs/types.ts:1465

***

### key

> **key**: `string`

Defined in: src/airs/types.ts:1462

***

### type

> **type**: `"VAR"` \| `"SECRET"`

Defined in: src/airs/types.ts:1464

***

### value?

> `optional` **value?**: `string` \| `null`

Defined in: src/airs/types.ts:1463
