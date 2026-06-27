# Function: writeBackupFile()

> **writeBackupFile**\<`T`\>(`dir`, `filename`, `envelope`, `format`): `void`

Defined in: [src/backup/io.ts:27](https://github.com/cdot65/prisma-airs-cli/blob/main/src/backup/io.ts#L27)

Serialize envelope and write to `dir/filename.{json|yaml}`. Creates dir if needed.

## Type Parameters

### T

`T`

## Parameters

### dir

`string`

### filename

`string`

### envelope

[`BackupEnvelope`](../interfaces/BackupEnvelope.md)\<`T`\>

### format

[`BackupFormat`](../type-aliases/BackupFormat.md)

## Returns

`void`
