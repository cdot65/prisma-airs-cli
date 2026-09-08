# Interface: RedTeamAdapterVar

Defined in: src/airs/types.ts:1466

An adapter configuration variable. Secrets are masked; key off `isRedacted`, not the value.

## Properties

### isRedacted?

> `optional` **isRedacted?**: `boolean`

Defined in: src/airs/types.ts:1470

***

### key

> **key**: `string`

Defined in: src/airs/types.ts:1467

***

### type

> **type**: `"VAR"` \| `"SECRET"`

Defined in: src/airs/types.ts:1469

***

### value?

> `optional` **value?**: `string` \| `null`

Defined in: src/airs/types.ts:1468
