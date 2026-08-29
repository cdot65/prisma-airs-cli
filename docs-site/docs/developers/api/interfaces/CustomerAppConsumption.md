# Interface: CustomerAppConsumption

Defined in: src/airs/types.ts:1142

Per-app consumption + violation snapshot, normalized from the SDK's dashboard endpoints.
Time window is fixed at construction.

## Properties

### appId

> **appId**: `string`

Defined in: src/airs/types.ts:1143

***

### appName

> **appName**: `string`

Defined in: src/airs/types.ts:1144

***

### cloud?

> `optional` **cloud?**: `string`

Defined in: src/airs/types.ts:1145

***

### detectors

> **detectors**: `object`[]

Defined in: src/airs/types.ts:1164

Per-detector violation severity counts, one entry per detection_type.

#### critical

> **critical**: `number`

#### high

> **high**: `number`

#### low

> **low**: `number`

#### medium

> **medium**: `number`

#### total

> **total**: `number`

#### type

> **type**: `string`

***

### monitoringSince?

> `optional` **monitoringSince?**: `string`

Defined in: src/airs/types.ts:1148

ISO timestamp of first monitoring (corresponds to SCM panel's "Monitoring Since").

***

### profiles

> **profiles**: `string`[]

Defined in: src/airs/types.ts:1150

Attached security profile names.

***

### sessions

> **sessions**: `object`

Defined in: src/airs/types.ts:1159

Session activity counts over the window.

#### total

> **total**: `number`

#### violating

> **violating**: `number`

***

### source?

> `optional` **source?**: `string`

Defined in: src/airs/types.ts:1146

***

### tokens

> **tokens**: `object`

Defined in: src/airs/types.ts:1152

Token consumption stats with scale qualifier (K = thousands, M = millions).

#### dailyAverage?

> `optional` **dailyAverage?**: `number`

#### dailyAverageScale?

> `optional` **dailyAverageScale?**: `string`

#### monthlyTotal?

> `optional` **monthlyTotal?**: `number`

#### monthlyTotalScale?

> `optional` **monthlyTotalScale?**: `string`

***

### totalViolating

> **totalViolating**: `number`

Defined in: src/airs/types.ts:1173

Sum of violating sessions across all detectors (mirrors SCM panel's badge).
