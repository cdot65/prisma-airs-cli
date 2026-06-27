# Function: sanitizeFilename()

> **sanitizeFilename**(`name`): `string`

Defined in: [src/backup/io.ts:10](https://github.com/cdot65/prisma-airs-cli/blob/main/src/backup/io.ts#L10)

Sanitize a resource name into a filesystem-safe filename (no extension).
Lowercases, replaces non-alphanumeric with hyphens, collapses runs, trims.

## Parameters

### name

`string`

## Returns

`string`
