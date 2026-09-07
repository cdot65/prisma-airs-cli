# Interface: RuntimeDailyReport

Defined in: src/reports/types.ts:99

Allowlisted, credential-free projection; never contains raw responses or scan content.

## Properties

### activity

> **activity**: `object`

Defined in: src/reports/types.ts:109

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

Defined in: src/reports/types.ts:104

***

### dailyTelemetry

> **dailyTelemetry**: [`ReportDailyTelemetry`](ReportDailyTelemetry.md)

Defined in: src/reports/types.ts:118

***

### findings

> **findings**: [`ReportFinding`](ReportFinding.md)[]

Defined in: src/reports/types.ts:108

***

### generatedAt

> **generatedAt**: `string`

Defined in: src/reports/types.ts:103

***

### health

> **health**: `"unknown"` \| `"attention"` \| `"review"` \| `"no-findings"`

Defined in: src/reports/types.ts:106

***

### limitations

> **limitations**: `string`[]

Defined in: src/reports/types.ts:119

***

### product

> **product**: `"Prisma AIRS AI Runtime Security"`

Defined in: src/reports/types.ts:101

***

### profiles

> **profiles**: [`ReportProfile`](ReportProfile.md)[]

Defined in: src/reports/types.ts:115

***

### registeredApps

> **registeredApps**: [`ReportRegisteredApp`](ReportRegisteredApp.md)[]

Defined in: src/reports/types.ts:116

***

### schemaVersion

> **schemaVersion**: `2`

Defined in: src/reports/types.ts:100

***

### sessions

> **sessions**: [`ReportSessionSummary`](ReportSessionSummary.md)

Defined in: src/reports/types.ts:117

***

### sources

> **sources**: [`ReportSource`](ReportSource.md)[]

Defined in: src/reports/types.ts:107

***

### title

> **title**: `string`

Defined in: src/reports/types.ts:102

***

### window

> **window**: `object`

Defined in: src/reports/types.ts:105

#### description

> **description**: `string`

#### end

> **end**: `string`

#### start

> **start**: `string`
