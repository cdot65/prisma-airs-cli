# Interface: RedTeamAdapterUpdateOverrides

Defined in: src/airs/types.ts:1514

CLI-side overrides for adapter update. The upstream PUT is a full
replacement, so the service merges these onto the current record —
`prompt` is the only always-required field because it is never stored.

## Properties

### description?

> `optional` **description?**: `string`

Defined in: src/airs/types.ts:1518

***

### name?

> `optional` **name?**: `string`

Defined in: src/airs/types.ts:1516

***

### networkBrokerChannelUuid?

> `optional` **networkBrokerChannelUuid?**: `string`

Defined in: src/airs/types.ts:1519

***

### prompt

> **prompt**: `string`

Defined in: src/airs/types.ts:1515

***

### scriptB64?

> `optional` **scriptB64?**: `string`

Defined in: src/airs/types.ts:1517

***

### variables?

> `optional` **variables?**: [`RedTeamAdapterVar`](RedTeamAdapterVar.md)[]

Defined in: src/airs/types.ts:1521

Replaces the WHOLE variable set when given; omitted keys are deleted upstream.
