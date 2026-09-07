# Interface: RedTeamEnvironmentReport

Defined in: src/reports/redteam.ts:24

Aggregate-only projection: no target credentials, identities, scripts, prompts or raw responses.

## Properties

### collectionStartedAt

> **collectionStartedAt**: `string`

Defined in: src/reports/redteam.ts:28

***

### findings

> **findings**: [`ReportFinding`](ReportFinding.md)[]

Defined in: src/reports/redteam.ts:33

***

### generatedAt

> **generatedAt**: `string`

Defined in: src/reports/redteam.ts:29

***

### health

> **health**: `"unknown"` \| `"attention"` \| `"review"` \| `"no-findings"`

Defined in: src/reports/redteam.ts:31

***

### limitations

> **limitations**: `string`[]

Defined in: src/reports/redteam.ts:35

***

### product

> **product**: `"Prisma AIRS AI Red Teaming"`

Defined in: src/reports/redteam.ts:26

***

### schemaVersion

> **schemaVersion**: `1`

Defined in: src/reports/redteam.ts:25

***

### sources

> **sources**: [`ReportSource`](ReportSource.md)[]

Defined in: src/reports/redteam.ts:32

***

### tables

> **tables**: [`RedTeamReportTable`](RedTeamReportTable.md)[]

Defined in: src/reports/redteam.ts:34

***

### title

> **title**: `string`

Defined in: src/reports/redteam.ts:27

***

### window

> **window**: `object`

Defined in: src/reports/redteam.ts:30

#### end

> **end**: `string`

#### start

> **start**: `string`
