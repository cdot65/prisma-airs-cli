# Interface: RuntimeDailyReport

Defined in: src/reports/types.ts:123

Allowlisted, credential-free projection; never contains raw responses or scan content.

## Properties

### activity

> **activity**: `object`

Defined in: src/reports/types.ts:133

#### applications

> **applications**: [`ReportApplication`](ReportApplication.md)[]

#### sessions

> **sessions**: `number` \| `null`

#### violatingSessions

> **violatingSessions**: `number` \| `null`

#### violationRate

> **violationRate**: `number` \| `null`

***

### collectionStartedAt

> **collectionStartedAt**: `string`

Defined in: src/reports/types.ts:128

***

### dailyTelemetry

> **dailyTelemetry**: [`ReportDailyTelemetry`](ReportDailyTelemetry.md)

Defined in: src/reports/types.ts:142

***

### findings

> **findings**: [`ReportFinding`](ReportFinding.md)[]

Defined in: src/reports/types.ts:132

***

### generatedAt

> **generatedAt**: `string`

Defined in: src/reports/types.ts:127

***

### health

> **health**: `"unknown"` \| `"attention"` \| `"review"` \| `"no-findings"`

Defined in: src/reports/types.ts:130

***

### limitations

> **limitations**: `string`[]

Defined in: src/reports/types.ts:143

***

### product

> **product**: `"Prisma AIRS AI Runtime Security"`

Defined in: src/reports/types.ts:125

***

### profiles

> **profiles**: [`ReportProfile`](ReportProfile.md)[]

Defined in: src/reports/types.ts:139

***

### registeredApps

> **registeredApps**: [`ReportRegisteredApp`](ReportRegisteredApp.md)[]

Defined in: src/reports/types.ts:140

***

### schemaVersion

> **schemaVersion**: `2`

Defined in: src/reports/types.ts:124

***

### sessions

> **sessions**: [`ReportSessionSummary`](ReportSessionSummary.md)

Defined in: src/reports/types.ts:141

***

### sources

> **sources**: [`ReportSource`](ReportSource.md)[]

Defined in: src/reports/types.ts:131

***

### title

> **title**: `string`

Defined in: src/reports/types.ts:126

***

### window

> **window**: `object`

Defined in: src/reports/types.ts:129

#### description

> **description**: `string`

#### end

> **end**: `string`

#### start

> **start**: `string`
