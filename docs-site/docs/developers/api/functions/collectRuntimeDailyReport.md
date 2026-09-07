# Function: collectRuntimeDailyReport()

> **collectRuntimeDailyReport**(`client`, `options?`): `Promise`\<[`RuntimeDailyReport`](../interfaces/RuntimeDailyReport.md)\>

Defined in: src/reports/runtime.ts:120

Build a daily environment report through SDK reads only. No scan submissions or mutations.
Application sessions, log entries and current inventory remain separate units of evidence.

## Parameters

### client

[`RuntimeReportClient`](../interfaces/RuntimeReportClient.md)

### options?

[`RuntimeReportOptions`](../interfaces/RuntimeReportOptions.md) = `{}`

## Returns

`Promise`\<[`RuntimeDailyReport`](../interfaces/RuntimeDailyReport.md)\>
