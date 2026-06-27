# Interface: PromptSetService

Defined in: [src/airs/types.ts:76](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L76)

Contract for custom prompt set operations in AI Red Team.

## Methods

### addPrompt()

> **addPrompt**(`promptSetId`, `prompt`, `goal?`): `Promise`\<\{ `prompt`: `string`; `uuid`: `string`; \}\>

Defined in: [src/airs/types.ts:80](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L80)

Add a prompt to an existing prompt set.

#### Parameters

##### promptSetId

`string`

##### prompt

`string`

##### goal?

`string`

#### Returns

`Promise`\<\{ `prompt`: `string`; `uuid`: `string`; \}\>

***

### archivePromptSet()

> **archivePromptSet**(`uuid`, `archive`): `Promise`\<`void`\>

Defined in: [src/airs/types.ts:95](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L95)

Archive or unarchive a prompt set.

#### Parameters

##### uuid

`string`

##### archive

`boolean`

#### Returns

`Promise`\<`void`\>

***

### createPromptSet()

> **createPromptSet**(`name`, `description?`): `Promise`\<\{ `name`: `string`; `uuid`: `string`; \}\>

Defined in: [src/airs/types.ts:78](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L78)

Create a new custom prompt set.

#### Parameters

##### name

`string`

##### description?

`string`

#### Returns

`Promise`\<\{ `name`: `string`; `uuid`: `string`; \}\>

***

### createPropertyName()

> **createPropertyName**(`name`): `Promise`\<[`MutationResponse`](MutationResponse.md)\>

Defined in: [src/airs/types.ts:117](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L117)

Create a property name.

#### Parameters

##### name

`string`

#### Returns

`Promise`\<[`MutationResponse`](MutationResponse.md)\>

***

### createPropertyValue()

> **createPropertyValue**(`name`, `value`): `Promise`\<[`MutationResponse`](MutationResponse.md)\>

Defined in: [src/airs/types.ts:121](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L121)

Create a property value.

#### Parameters

##### name

`string`

##### value

`string`

#### Returns

`Promise`\<[`MutationResponse`](MutationResponse.md)\>

***

### deletePrompt()

> **deletePrompt**(`setUuid`, `promptUuid`): `Promise`\<`void`\>

Defined in: [src/airs/types.ts:113](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L113)

Delete a prompt.

#### Parameters

##### setUuid

`string`

##### promptUuid

`string`

#### Returns

`Promise`\<`void`\>

***

### downloadTemplate()

> **downloadTemplate**(`uuid`): `Promise`\<`string`\>

Defined in: [src/airs/types.ts:99](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L99)

Download CSV template for a prompt set.

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<`string`\>

***

### getPrompt()

> **getPrompt**(`setUuid`, `promptUuid`): `Promise`\<[`PromptDetail`](PromptDetail.md)\>

Defined in: [src/airs/types.ts:105](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L105)

Get a single prompt.

#### Parameters

##### setUuid

`string`

##### promptUuid

`string`

#### Returns

`Promise`\<[`PromptDetail`](PromptDetail.md)\>

***

### getPromptSet()

> **getPromptSet**(`uuid`): `Promise`\<[`PromptSetDetail`](PromptSetDetail.md)\>

Defined in: [src/airs/types.ts:88](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L88)

Get prompt set details.

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<[`PromptSetDetail`](PromptSetDetail.md)\>

***

### getPromptSetVersionInfo()

> **getPromptSetVersionInfo**(`uuid`): `Promise`\<[`PromptSetVersionInfo`](PromptSetVersionInfo.md)\>

Defined in: [src/airs/types.ts:97](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L97)

Get prompt set version info with stats.

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<[`PromptSetVersionInfo`](PromptSetVersionInfo.md)\>

***

### getPropertyNames()

> **getPropertyNames**(): `Promise`\<`string`[]\>

Defined in: [src/airs/types.ts:115](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L115)

List property names.

#### Returns

`Promise`\<`string`[]\>

***

### getPropertyValues()

> **getPropertyValues**(`name`): `Promise`\<[`PropertyValueList`](PropertyValueList.md)\>

Defined in: [src/airs/types.ts:119](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L119)

Get values for a property.

#### Parameters

##### name

`string`

#### Returns

`Promise`\<[`PropertyValueList`](PropertyValueList.md)\>

***

### listPrompts()

> **listPrompts**(`setUuid`, `opts?`): `Promise`\<[`PromptDetail`](PromptDetail.md)[]\>

Defined in: [src/airs/types.ts:103](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L103)

List prompts in a prompt set.

#### Parameters

##### setUuid

`string`

##### opts?

###### limit?

`number`

###### skip?

`number`

#### Returns

`Promise`\<[`PromptDetail`](PromptDetail.md)[]\>

***

### listPromptSets()

> **listPromptSets**(): `Promise`\<`object`[]\>

Defined in: [src/airs/types.ts:86](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L86)

List all custom prompt sets.

#### Returns

`Promise`\<`object`[]\>

***

### updatePrompt()

> **updatePrompt**(`setUuid`, `promptUuid`, `request`): `Promise`\<[`PromptDetail`](PromptDetail.md)\>

Defined in: [src/airs/types.ts:107](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L107)

Update a prompt.

#### Parameters

##### setUuid

`string`

##### promptUuid

`string`

##### request

###### goal?

`string`

###### prompt?

`string`

#### Returns

`Promise`\<[`PromptDetail`](PromptDetail.md)\>

***

### updatePromptSet()

> **updatePromptSet**(`uuid`, `request`): `Promise`\<[`PromptSetDetail`](PromptSetDetail.md)\>

Defined in: [src/airs/types.ts:90](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L90)

Update prompt set name/description.

#### Parameters

##### uuid

`string`

##### request

###### description?

`string`

###### name?

`string`

#### Returns

`Promise`\<[`PromptSetDetail`](PromptSetDetail.md)\>

***

### uploadPromptsCsv()

> **uploadPromptsCsv**(`uuid`, `file`): `Promise`\<\{ `message`: `string`; `status`: `number`; \}\>

Defined in: [src/airs/types.ts:101](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L101)

Upload CSV file to a prompt set.

#### Parameters

##### uuid

`string`

##### file

`Blob`

#### Returns

`Promise`\<\{ `message`: `string`; `status`: `number`; \}\>
