# Interface: RedTeamAdapterUpdateOverrides

Defined in: prisma-airs-cli/src/airs/types.ts:1517

CLI-side overrides for adapter update. The upstream PUT is a full
replacement, so the service merges these onto the current record —
`prompt` is the only always-required field because it is never stored.

## Properties

### description?

> `optional` **description?**: `string`

Defined in: prisma-airs-cli/src/airs/types.ts:1521

***

### name?

> `optional` **name?**: `string`

Defined in: prisma-airs-cli/src/airs/types.ts:1519

***

### networkBrokerChannelUuid?

> `optional` **networkBrokerChannelUuid?**: `string`

Defined in: prisma-airs-cli/src/airs/types.ts:1522

***

### prompt

> **prompt**: `string`

Defined in: prisma-airs-cli/src/airs/types.ts:1518

***

### scriptB64?

> `optional` **scriptB64?**: `string`

Defined in: prisma-airs-cli/src/airs/types.ts:1520

***

### variables?

> `optional` **variables?**: [`RedTeamAdapterVar`](RedTeamAdapterVar.md)[]

Defined in: prisma-airs-cli/src/airs/types.ts:1524

Replaces the WHOLE variable set when given; omitted keys are deleted upstream.
