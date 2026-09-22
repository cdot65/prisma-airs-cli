# Interface: RuntimeReportOptions

Defined in: src/reports/types.ts:146

## Properties

### maxPages?

> `optional` **maxPages?**: `number`

Defined in: src/reports/types.ts:149

Per-source page budget (1–100); partial results are explicitly marked. Default 40.

***

### now?

> `optional` **now?**: () => `Date`

Defined in: src/reports/types.ts:151

Inject a clock for deterministic tests.

#### Returns

`Date`

***

### title?

> `optional` **title?**: `string`

Defined in: src/reports/types.ts:147
