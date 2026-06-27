# Interface: RestoreResult

Defined in: [src/backup/types.ts:24](https://github.com/cdot65/prisma-airs-cli/blob/main/src/backup/types.ts#L24)

Per-target result reported after a restore run.

## Properties

### action

> **action**: `"failed"` \| `"created"` \| `"updated"` \| `"skipped"`

Defined in: [src/backup/types.ts:26](https://github.com/cdot65/prisma-airs-cli/blob/main/src/backup/types.ts#L26)

***

### error?

> `optional` **error?**: `string`

Defined in: [src/backup/types.ts:27](https://github.com/cdot65/prisma-airs-cli/blob/main/src/backup/types.ts#L27)

***

### name

> **name**: `string`

Defined in: [src/backup/types.ts:25](https://github.com/cdot65/prisma-airs-cli/blob/main/src/backup/types.ts#L25)
