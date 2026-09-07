# Interface: ReportDailyTelemetry

Defined in: src/reports/types.ts:79

## Properties

### chart

> **chart**: `object`

Defined in: src/reports/types.ts:80

#### buckets

> **buckets**: `object`[]

#### sessions

> **sessions**: `number` \| `null`

#### violatingSessions

> **violatingSessions**: `number` \| `null`

***

### topApplications

> **topApplications**: `object`[]

Defined in: src/reports/types.ts:90

#### detectors

> **detectors**: `object`[]

#### name

> **name**: `string`

#### violations

> **violations**: `number` \| `null`

***

### violationTrend

> **violationTrend**: `object`[]

Defined in: src/reports/types.ts:95

#### time

> **time**: `string`

#### violations

> **violations**: [`ReportSeverity`](ReportSeverity.md)
