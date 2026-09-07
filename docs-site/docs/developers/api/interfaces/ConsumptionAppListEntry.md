# Interface: ConsumptionAppListEntry

Defined in: prisma-airs-cli/src/airs/types.ts:1195

One entry from the dashboard's apps-overview enumeration.

One per dashboard bucket. A single registered customer-app can produce multiple buckets when
scan payloads sent under its API key carry different `metadata.app_name` values - the
dashboard tracks each as its own bucket. The `id` field is the registered `customer_appId`
UUID; the `name` field is the literal scan-payload value.

## Properties

### appId

> **appId**: `string`

Defined in: prisma-airs-cli/src/airs/types.ts:1197

Registered customer_appId UUID.

***

### appName

> **appName**: `string`

Defined in: prisma-airs-cli/src/airs/types.ts:1199

Dashboard bucket name (literal scan-payload `metadata.app_name`).

***

### cloud?

> `optional` **cloud?**: `string`

Defined in: prisma-airs-cli/src/airs/types.ts:1201

Cloud provider tag, if reported by the dashboard.

***

### source?

> `optional` **source?**: `string`

Defined in: prisma-airs-cli/src/airs/types.ts:1203

Origin of the bucket, e.g. 'api'.
