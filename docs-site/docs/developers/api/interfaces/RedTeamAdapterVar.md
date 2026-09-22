# Interface: RedTeamAdapterVar

Defined in: src/airs/types.ts:1514

An adapter configuration variable. Secrets are masked; key off `isRedacted`, not the value.

## Properties

### isRedacted?

> `optional` **isRedacted?**: `boolean`

Defined in: src/airs/types.ts:1518

***

### key

> **key**: `string`

Defined in: src/airs/types.ts:1515

***

### type

> **type**: `"VAR"` \| `"SECRET"`

Defined in: src/airs/types.ts:1517

***

### value?

> `optional` **value?**: `string` \| `null`

Defined in: src/airs/types.ts:1516
