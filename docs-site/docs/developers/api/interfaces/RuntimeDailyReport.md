# Interface: RuntimeDailyReport

Defined in: src/reports/types.ts:67

Allowlisted, credential-free projection; never contains raw responses or scan content.

## Properties

### activity

> **activity**: `object`

Defined in: src/reports/types.ts:77

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

Defined in: src/reports/types.ts:72

***

### findings

> **findings**: [`ReportFinding`](ReportFinding.md)[]

Defined in: src/reports/types.ts:76

***

### generatedAt

> **generatedAt**: `string`

Defined in: src/reports/types.ts:71

***

### health

> **health**: `"unknown"` \| `"attention"` \| `"review"` \| `"no-findings"`

Defined in: src/reports/types.ts:74

***

### limitations

> **limitations**: `string`[]

Defined in: src/reports/types.ts:86

***

### logs

> **logs**: [`ReportLogSummary`](ReportLogSummary.md)

Defined in: src/reports/types.ts:85

***

### product

> **product**: `"Prisma AIRS AI Runtime Security"`

Defined in: src/reports/types.ts:69

***

### profiles

> **profiles**: [`ReportProfile`](ReportProfile.md)[]

Defined in: src/reports/types.ts:83

***

### registeredApps

> **registeredApps**: [`ReportRegisteredApp`](ReportRegisteredApp.md)[]

Defined in: src/reports/types.ts:84

***

### schemaVersion

> **schemaVersion**: `1`

Defined in: src/reports/types.ts:68

***

### sources

> **sources**: [`ReportSource`](ReportSource.md)[]

Defined in: src/reports/types.ts:75

***

### title

> **title**: `string`

Defined in: src/reports/types.ts:70

***

### window

> **window**: `object`

Defined in: src/reports/types.ts:73

#### description

> **description**: `string`

#### end

> **end**: `string`

#### start

> **start**: `string`
