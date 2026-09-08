# Interface: EnvironmentReport

Defined in: src/reports/types.ts:29

Aggregate-only view shared by portable product dashboards.

## Properties

### collectionStartedAt

> **collectionStartedAt**: `string`

Defined in: src/reports/types.ts:33

***

### findings

> **findings**: [`ReportFinding`](ReportFinding.md)[]

Defined in: src/reports/types.ts:40

***

### generatedAt

> **generatedAt**: `string`

Defined in: src/reports/types.ts:34

***

### health

> **health**: `"unknown"` \| `"attention"` \| `"review"` \| `"no-findings"`

Defined in: src/reports/types.ts:38

***

### limitations

> **limitations**: `string`[]

Defined in: src/reports/types.ts:42

***

### product

> **product**: `string`

Defined in: src/reports/types.ts:31

***

### schemaLabel?

> `optional` **schemaLabel?**: `string`

Defined in: src/reports/types.ts:37

***

### schemaVersion

> **schemaVersion**: `1`

Defined in: src/reports/types.ts:30

***

### sources

> **sources**: [`ReportSource`](ReportSource.md)[]

Defined in: src/reports/types.ts:39

***

### tables

> **tables**: [`EnvironmentReportTable`](EnvironmentReportTable.md)[]

Defined in: src/reports/types.ts:41

***

### title

> **title**: `string`

Defined in: src/reports/types.ts:32

***

### window

> **window**: `object`

Defined in: src/reports/types.ts:35

#### end

> **end**: `string`

#### start

> **start**: `string`

***

### windowLabel?

> `optional` **windowLabel?**: `string`

Defined in: src/reports/types.ts:36
