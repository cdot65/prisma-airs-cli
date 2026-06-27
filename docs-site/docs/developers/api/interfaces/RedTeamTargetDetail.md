# Interface: RedTeamTargetDetail

Defined in: [src/airs/types.ts:153](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L153)

Detailed target info with connection params and metadata.

## Extends

- [`RedTeamTarget`](RedTeamTarget.md)

## Properties

### active

> **active**: `boolean`

Defined in: [src/airs/types.ts:149](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L149)

#### Inherited from

[`RedTeamTarget`](RedTeamTarget.md).[`active`](RedTeamTarget.md#active)

***

### additionalContext?

> `optional` **additionalContext?**: `object`

Defined in: [src/airs/types.ts:169](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L169)

#### documents?

> `optional` **documents?**: `unknown`[] \| `null`

#### system\_prompt?

> `optional` **system\_prompt?**: `string` \| `null`

#### use\_case\_description?

> `optional` **use\_case\_description?**: `string` \| `null`

***

### apiEndpointType?

> `optional` **apiEndpointType?**: `string` \| `null`

Defined in: [src/airs/types.ts:155](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L155)

***

### authConfig?

> `optional` **authConfig?**: `Record`\<`string`, `unknown`\> \| `null`

Defined in: [src/airs/types.ts:158](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L158)

***

### authType?

> `optional` **authType?**: `string` \| `null`

Defined in: [src/airs/types.ts:157](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L157)

***

### background?

> `optional` **background?**: `object`

Defined in: [src/airs/types.ts:164](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L164)

#### competitors?

> `optional` **competitors?**: `string`[] \| `null`

#### industry?

> `optional` **industry?**: `string` \| `null`

#### use\_case?

> `optional` **use\_case?**: `string` \| `null`

***

### connectionParams?

> `optional` **connectionParams?**: `Record`\<`string`, `unknown`\>

Defined in: [src/airs/types.ts:163](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L163)

***

### connectionType?

> `optional` **connectionType?**: `string` \| `null`

Defined in: [src/airs/types.ts:154](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L154)

***

### description?

> `optional` **description?**: `string` \| `null`

Defined in: [src/airs/types.ts:162](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L162)

***

### extraInfo?

> `optional` **extraInfo?**: `Record`\<`string`, `unknown`\> \| `null`

Defined in: [src/airs/types.ts:161](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L161)

***

### metadata?

> `optional` **metadata?**: `object`

Defined in: [src/airs/types.ts:174](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L174)

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

Defined in: [src/airs/types.ts:146](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L146)

#### Inherited from

[`RedTeamTarget`](RedTeamTarget.md).[`name`](RedTeamTarget.md#name)

***

### networkBrokerChannelUuid?

> `optional` **networkBrokerChannelUuid?**: `string` \| `null`

Defined in: [src/airs/types.ts:159](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L159)

***

### responseMode?

> `optional` **responseMode?**: `string` \| `null`

Defined in: [src/airs/types.ts:156](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L156)

***

### sessionSupported?

> `optional` **sessionSupported?**: `boolean`

Defined in: [src/airs/types.ts:160](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L160)

***

### status

> **status**: `string`

Defined in: [src/airs/types.ts:147](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L147)

#### Inherited from

[`RedTeamTarget`](RedTeamTarget.md).[`status`](RedTeamTarget.md#status)

***

### targetType?

> `optional` **targetType?**: `string`

Defined in: [src/airs/types.ts:148](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L148)

#### Inherited from

[`RedTeamTarget`](RedTeamTarget.md).[`targetType`](RedTeamTarget.md#targettype)

***

### uuid

> **uuid**: `string`

Defined in: [src/airs/types.ts:145](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L145)

#### Inherited from

[`RedTeamTarget`](RedTeamTarget.md).[`uuid`](RedTeamTarget.md#uuid)
