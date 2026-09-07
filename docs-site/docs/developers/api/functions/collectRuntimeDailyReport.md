# Function: collectRuntimeDailyReport()

> **collectRuntimeDailyReport**(`client`, `options?`): `Promise`\<[`RuntimeDailyReport`](../interfaces/RuntimeDailyReport.md)\>

Defined in: src/reports/runtime.ts:135

Build a daily environment report through SDK reads only. No scan submissions or mutations.
Application buckets, session inventory and detector violations remain separate units of evidence.

## Parameters

### client

[`RuntimeReportClient`](../interfaces/RuntimeReportClient.md)

### options?

[`RuntimeReportOptions`](../interfaces/RuntimeReportOptions.md) = `{}`

## Returns

`Promise`\<[`RuntimeDailyReport`](../interfaces/RuntimeDailyReport.md)\>
