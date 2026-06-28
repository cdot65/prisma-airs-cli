# Interface: RedTeamTargetDetail

Defined in: [src/airs/types.ts:161](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L161)

Detailed target info with connection params and metadata.

## Extends

- [`RedTeamTarget`](RedTeamTarget.md)

## Properties

### active

> **active**: `boolean`

Defined in: [src/airs/types.ts:157](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L157)

#### Inherited from

[`RedTeamTarget`](RedTeamTarget.md).[`active`](RedTeamTarget.md#active)

***

### additionalContext?

> `optional` **additionalContext?**: `object`

Defined in: [src/airs/types.ts:177](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L177)

#### documents?

> `optional` **documents?**: `unknown`[] \| `null`

#### system\_prompt?

> `optional` **system\_prompt?**: `string` \| `null`

#### use\_case\_description?

> `optional` **use\_case\_description?**: `string` \| `null`

***

### apiEndpointType?

> `optional` **apiEndpointType?**: `string` \| `null`

Defined in: [src/airs/types.ts:163](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L163)

***

### authConfig?

> `optional` **authConfig?**: `Record`\<`string`, `unknown`\> \| `null`

Defined in: [src/airs/types.ts:166](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L166)

***

### authType?

> `optional` **authType?**: `string` \| `null`

Defined in: [src/airs/types.ts:165](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L165)

***

### background?

> `optional` **background?**: `object`

Defined in: [src/airs/types.ts:172](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L172)

#### competitors?

> `optional` **competitors?**: `string`[] \| `null`

#### industry?

> `optional` **industry?**: `string` \| `null`

#### use\_case?

> `optional` **use\_case?**: `string` \| `null`

***

### connectionParams?

> `optional` **connectionParams?**: `Record`\<`string`, `unknown`\>

Defined in: [src/airs/types.ts:171](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L171)

***

### connectionType?

> `optional` **connectionType?**: `string` \| `null`

Defined in: [src/airs/types.ts:162](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L162)

***

### description?

> `optional` **description?**: `string` \| `null`

Defined in: [src/airs/types.ts:170](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L170)

***

### extraInfo?

> `optional` **extraInfo?**: `Record`\<`string`, `unknown`\> \| `null`

Defined in: [src/airs/types.ts:169](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L169)

***

### metadata?

> `optional` **metadata?**: `object`

Defined in: [src/airs/types.ts:182](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L182)

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

Defined in: [src/airs/types.ts:154](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L154)

#### Inherited from

[`RedTeamTarget`](RedTeamTarget.md).[`name`](RedTeamTarget.md#name)

***

### networkBrokerChannelUuid?

> `optional` **networkBrokerChannelUuid?**: `string` \| `null`

Defined in: [src/airs/types.ts:167](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L167)

***

### responseMode?

> `optional` **responseMode?**: `string` \| `null`

Defined in: [src/airs/types.ts:164](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L164)

***

### sessionSupported?

> `optional` **sessionSupported?**: `boolean`

Defined in: [src/airs/types.ts:168](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L168)

***

### status

> **status**: `string`

Defined in: [src/airs/types.ts:155](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L155)

#### Inherited from

[`RedTeamTarget`](RedTeamTarget.md).[`status`](RedTeamTarget.md#status)

***

### targetType?

> `optional` **targetType?**: `string`

Defined in: [src/airs/types.ts:156](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L156)

#### Inherited from

[`RedTeamTarget`](RedTeamTarget.md).[`targetType`](RedTeamTarget.md#targettype)

***

### uuid

> **uuid**: `string`

Defined in: [src/airs/types.ts:153](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts#L153)

#### Inherited from

[`RedTeamTarget`](RedTeamTarget.md).[`uuid`](RedTeamTarget.md#uuid)
