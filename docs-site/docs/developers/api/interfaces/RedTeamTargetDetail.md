# Interface: RedTeamTargetDetail

Defined in: src/airs/types.ts:224

Detailed target info with connection params and metadata.

## Extends

- [`RedTeamTarget`](RedTeamTarget.md)

## Properties

### active

> **active**: `boolean`

Defined in: src/airs/types.ts:220

#### Inherited from

[`RedTeamTarget`](RedTeamTarget.md).[`active`](RedTeamTarget.md#active)

***

### additionalContext?

> `optional` **additionalContext?**: `object`

Defined in: src/airs/types.ts:240

#### documents?

> `optional` **documents?**: `unknown`[] \| `null`

#### system\_prompt?

> `optional` **system\_prompt?**: `string` \| `null`

#### use\_case\_description?

> `optional` **use\_case\_description?**: `string` \| `null`

***

### apiEndpointType?

> `optional` **apiEndpointType?**: `string` \| `null`

Defined in: src/airs/types.ts:226

***

### authConfig?

> `optional` **authConfig?**: `Record`\<`string`, `unknown`\> \| `null`

Defined in: src/airs/types.ts:229

***

### authType?

> `optional` **authType?**: `string` \| `null`

Defined in: src/airs/types.ts:228

***

### background?

> `optional` **background?**: `object`

Defined in: src/airs/types.ts:235

#### competitors?

> `optional` **competitors?**: `string`[] \| `null`

#### industry?

> `optional` **industry?**: `string` \| `null`

#### use\_case?

> `optional` **use\_case?**: `string` \| `null`

***

### connectionParams?

> `optional` **connectionParams?**: `Record`\<`string`, `unknown`\>

Defined in: src/airs/types.ts:234

***

### connectionType?

> `optional` **connectionType?**: `string` \| `null`

Defined in: src/airs/types.ts:225

***

### description?

> `optional` **description?**: `string` \| `null`

Defined in: src/airs/types.ts:233

***

### extraInfo?

> `optional` **extraInfo?**: `Record`\<`string`, `unknown`\> \| `null`

Defined in: src/airs/types.ts:232

***

### metadata?

> `optional` **metadata?**: `object`

Defined in: src/airs/types.ts:245

#### api\_endpoint\_type?

> `optional` **api\_endpoint\_type?**: `string` \| `null`

#### is\_streaming\_enabled?

> `optional` **is\_streaming\_enabled?**: `boolean` \| `null`

#### max\_turns?

> `optional` **max\_turns?**: `number` \| `null`

#### multi\_turn?

> `optional` **multi\_turn?**: `boolean`

#### rate\_limit?

> `optional` **rate\_limit?**: `number` \| `null`

#### rate\_limit\_error\_json?

> `optional` **rate\_limit\_error\_json?**: `Record`\<`string`, `unknown`\> \| `null`

#### response\_mode?

> `optional` **response\_mode?**: `string` \| `null`

***

### name

> **name**: `string`

Defined in: src/airs/types.ts:217

#### Inherited from

[`RedTeamTarget`](RedTeamTarget.md).[`name`](RedTeamTarget.md#name)

***

### networkBrokerChannelUuid?

> `optional` **networkBrokerChannelUuid?**: `string` \| `null`

Defined in: src/airs/types.ts:230

***

### responseMode?

> `optional` **responseMode?**: `string` \| `null`

Defined in: src/airs/types.ts:227

***

### sessionSupported?

> `optional` **sessionSupported?**: `boolean`

Defined in: src/airs/types.ts:231

***

### status

> **status**: `string`

Defined in: src/airs/types.ts:218

#### Inherited from

[`RedTeamTarget`](RedTeamTarget.md).[`status`](RedTeamTarget.md#status)

***

### targetType?

> `optional` **targetType?**: `string`

Defined in: src/airs/types.ts:219

#### Inherited from

[`RedTeamTarget`](RedTeamTarget.md).[`targetType`](RedTeamTarget.md#targettype)

***

### uuid

> **uuid**: `string`

Defined in: src/airs/types.ts:216

#### Inherited from

[`RedTeamTarget`](RedTeamTarget.md).[`uuid`](RedTeamTarget.md#uuid)
