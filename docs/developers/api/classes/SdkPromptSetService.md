# Class: SdkPromptSetService

Defined in: [src/airs/promptsets.ts:39](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/promptsets.ts#L39)

Wraps the SDK's RedTeamClient.customAttacks to implement PromptSetService.
Creates and populates custom prompt sets for AI Red Team.

## Implements

- [`PromptSetService`](../interfaces/PromptSetService.md)

## Constructors

### Constructor

> **new SdkPromptSetService**(`opts?`): `SdkPromptSetService`

Defined in: [src/airs/promptsets.ts:42](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/promptsets.ts#L42)

#### Parameters

##### opts?

`RedTeamClientOptions`

#### Returns

`SdkPromptSetService`

## Methods

### addPrompt()

> **addPrompt**(`promptSetId`, `prompt`, `goal?`): `Promise`\<\{ `prompt`: `string`; `uuid`: `string`; \}\>

Defined in: [src/airs/promptsets.ts:57](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/promptsets.ts#L57)

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

#### Implementation of

[`PromptSetService`](../interfaces/PromptSetService.md).[`addPrompt`](../interfaces/PromptSetService.md#addprompt)

***

### archivePromptSet()

> **archivePromptSet**(`uuid`, `archive`): `Promise`\<`void`\>

Defined in: [src/airs/promptsets.ts:92](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/promptsets.ts#L92)

Archive or unarchive a prompt set.

#### Parameters

##### uuid

`string`

##### archive

`boolean`

#### Returns

`Promise`\<`void`\>

#### Implementation of

[`PromptSetService`](../interfaces/PromptSetService.md).[`archivePromptSet`](../interfaces/PromptSetService.md#archivepromptset)

***

### createPromptSet()

> **createPromptSet**(`name`, `description?`): `Promise`\<\{ `name`: `string`; `uuid`: `string`; \}\>

Defined in: [src/airs/promptsets.ts:46](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/promptsets.ts#L46)

Create a new custom prompt set.

#### Parameters

##### name

`string`

##### description?

`string`

#### Returns

`Promise`\<\{ `name`: `string`; `uuid`: `string`; \}\>

#### Implementation of

[`PromptSetService`](../interfaces/PromptSetService.md).[`createPromptSet`](../interfaces/PromptSetService.md#createpromptset)

***

### createPropertyName()

> **createPropertyName**(`name`): `Promise`\<[`MutationResponse`](../interfaces/MutationResponse.md)\>

Defined in: [src/airs/promptsets.ts:153](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/promptsets.ts#L153)

Create a property name.

#### Parameters

##### name

`string`

#### Returns

`Promise`\<[`MutationResponse`](../interfaces/MutationResponse.md)\>

#### Implementation of

[`PromptSetService`](../interfaces/PromptSetService.md).[`createPropertyName`](../interfaces/PromptSetService.md#createpropertyname)

***

### createPropertyValue()

> **createPropertyValue**(`name`, `value`): `Promise`\<[`MutationResponse`](../interfaces/MutationResponse.md)\>

Defined in: [src/airs/promptsets.ts:164](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/promptsets.ts#L164)

Create a property value.

#### Parameters

##### name

`string`

##### value

`string`

#### Returns

`Promise`\<[`MutationResponse`](../interfaces/MutationResponse.md)\>

#### Implementation of

[`PromptSetService`](../interfaces/PromptSetService.md).[`createPropertyValue`](../interfaces/PromptSetService.md#createpropertyvalue)

***

### deletePrompt()

> **deletePrompt**(`setUuid`, `promptUuid`): `Promise`\<`void`\>

Defined in: [src/airs/promptsets.ts:143](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/promptsets.ts#L143)

Delete a prompt.

#### Parameters

##### setUuid

`string`

##### promptUuid

`string`

#### Returns

`Promise`\<`void`\>

#### Implementation of

[`PromptSetService`](../interfaces/PromptSetService.md).[`deletePrompt`](../interfaces/PromptSetService.md#deleteprompt)

***

### downloadTemplate()

> **downloadTemplate**(`uuid`): `Promise`\<`string`\>

Defined in: [src/airs/promptsets.ts:106](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/promptsets.ts#L106)

Download CSV template for a prompt set.

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<`string`\>

#### Implementation of

[`PromptSetService`](../interfaces/PromptSetService.md).[`downloadTemplate`](../interfaces/PromptSetService.md#downloadtemplate)

***

### getPrompt()

> **getPrompt**(`setUuid`, `promptUuid`): `Promise`\<[`PromptDetail`](../interfaces/PromptDetail.md)\>

Defined in: [src/airs/promptsets.ts:125](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/promptsets.ts#L125)

Get a single prompt.

#### Parameters

##### setUuid

`string`

##### promptUuid

`string`

#### Returns

`Promise`\<[`PromptDetail`](../interfaces/PromptDetail.md)\>

#### Implementation of

[`PromptSetService`](../interfaces/PromptSetService.md).[`getPrompt`](../interfaces/PromptSetService.md#getprompt)

***

### getPromptSet()

> **getPromptSet**(`uuid`): `Promise`\<[`PromptSetDetail`](../interfaces/PromptSetDetail.md)\>

Defined in: [src/airs/promptsets.ts:79](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/promptsets.ts#L79)

Get prompt set details.

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<[`PromptSetDetail`](../interfaces/PromptSetDetail.md)\>

#### Implementation of

[`PromptSetService`](../interfaces/PromptSetService.md).[`getPromptSet`](../interfaces/PromptSetService.md#getpromptset)

***

### getPromptSetVersionInfo()

> **getPromptSetVersionInfo**(`uuid`): `Promise`\<[`PromptSetVersionInfo`](../interfaces/PromptSetVersionInfo.md)\>

Defined in: [src/airs/promptsets.ts:96](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/promptsets.ts#L96)

Get prompt set version info with stats.

#### Parameters

##### uuid

`string`

#### Returns

`Promise`\<[`PromptSetVersionInfo`](../interfaces/PromptSetVersionInfo.md)\>

#### Implementation of

[`PromptSetService`](../interfaces/PromptSetService.md).[`getPromptSetVersionInfo`](../interfaces/PromptSetService.md#getpromptsetversioninfo)

***

### getPropertyNames()

> **getPropertyNames**(): `Promise`\<`string`[]\>

Defined in: [src/airs/promptsets.ts:147](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/promptsets.ts#L147)

List property names.

#### Returns

`Promise`\<`string`[]\>

#### Implementation of

[`PromptSetService`](../interfaces/PromptSetService.md).[`getPropertyNames`](../interfaces/PromptSetService.md#getpropertynames)

***

### getPropertyValues()

> **getPropertyValues**(`name`): `Promise`\<[`PropertyValueList`](../interfaces/PropertyValueList.md)\>

Defined in: [src/airs/promptsets.ts:158](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/promptsets.ts#L158)

Get values for a property.

#### Parameters

##### name

`string`

#### Returns

`Promise`\<[`PropertyValueList`](../interfaces/PropertyValueList.md)\>

#### Implementation of

[`PromptSetService`](../interfaces/PromptSetService.md).[`getPropertyValues`](../interfaces/PromptSetService.md#getpropertyvalues)

***

### listPrompts()

> **listPrompts**(`setUuid`, `opts?`): `Promise`\<[`PromptDetail`](../interfaces/PromptDetail.md)[]\>

Defined in: [src/airs/promptsets.ts:115](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/promptsets.ts#L115)

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

`Promise`\<[`PromptDetail`](../interfaces/PromptDetail.md)[]\>

#### Implementation of

[`PromptSetService`](../interfaces/PromptSetService.md).[`listPrompts`](../interfaces/PromptSetService.md#listprompts)

***

### listPromptSets()

> **listPromptSets**(): `Promise`\<`object`[]\>

Defined in: [src/airs/promptsets.ts:70](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/promptsets.ts#L70)

List all custom prompt sets.

#### Returns

`Promise`\<`object`[]\>

#### Implementation of

[`PromptSetService`](../interfaces/PromptSetService.md).[`listPromptSets`](../interfaces/PromptSetService.md#listpromptsets)

***

### updatePrompt()

> **updatePrompt**(`setUuid`, `promptUuid`, `request`): `Promise`\<[`PromptDetail`](../interfaces/PromptDetail.md)\>

Defined in: [src/airs/promptsets.ts:130](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/promptsets.ts#L130)

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

`Promise`\<[`PromptDetail`](../interfaces/PromptDetail.md)\>

#### Implementation of

[`PromptSetService`](../interfaces/PromptSetService.md).[`updatePrompt`](../interfaces/PromptSetService.md#updateprompt)

***

### updatePromptSet()

> **updatePromptSet**(`uuid`, `request`): `Promise`\<[`PromptSetDetail`](../interfaces/PromptSetDetail.md)\>

Defined in: [src/airs/promptsets.ts:84](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/promptsets.ts#L84)

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

`Promise`\<[`PromptSetDetail`](../interfaces/PromptSetDetail.md)\>

#### Implementation of

[`PromptSetService`](../interfaces/PromptSetService.md).[`updatePromptSet`](../interfaces/PromptSetService.md#updatepromptset)

***

### uploadPromptsCsv()

> **uploadPromptsCsv**(`uuid`, `file`): `Promise`\<\{ `message`: `string`; `status`: `number`; \}\>

Defined in: [src/airs/promptsets.ts:110](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/promptsets.ts#L110)

Upload CSV file to a prompt set.

#### Parameters

##### uuid

`string`

##### file

`Blob`

#### Returns

`Promise`\<\{ `message`: `string`; `status`: `number`; \}\>

#### Implementation of

[`PromptSetService`](../interfaces/PromptSetService.md).[`uploadPromptsCsv`](../interfaces/PromptSetService.md#uploadpromptscsv)
