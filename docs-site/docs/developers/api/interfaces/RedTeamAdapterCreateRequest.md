# Interface: RedTeamAdapterCreateRequest

Defined in: src/airs/types.ts:1502

## Properties

### description?

> `optional` **description?**: `string`

Defined in: src/airs/types.ts:1507

***

### name

> **name**: `string`

Defined in: src/airs/types.ts:1503

***

### networkBrokerChannelUuid?

> `optional` **networkBrokerChannelUuid?**: `string`

Defined in: src/airs/types.ts:1509

Optional while DRAFT; required to activate (validate: true).

***

### prompt

> **prompt**: `string`

Defined in: src/airs/types.ts:1506

Sample prompt used to exercise the adapter during validation. Not stored.

***

### scriptB64

> **scriptB64**: `string`

Defined in: src/airs/types.ts:1504

***

### variables?

> `optional` **variables?**: [`RedTeamAdapterVar`](RedTeamAdapterVar.md)[]

Defined in: src/airs/types.ts:1510
