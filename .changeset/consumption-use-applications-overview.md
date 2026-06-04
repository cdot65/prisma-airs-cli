---
'@cdot65/prisma-airs-cli': patch
---

Fix `airs runtime customer-apps consumption` returning zero results when an integration overrides `metadata.app_name` in scan payloads (e.g. LiteLLM's `panw_prisma_airs` guardrail).

The dashboard buckets traffic by the literal scan-payload `metadata.app_name`, not by the SCM-registered customer-app name. A single registered customer-app can therefore have multiple dashboard buckets, one per distinct name an integration has sent under its API key. The previous implementation enumerated from `customerApps.list` and silently dropped every bucket whose name didn't exactly match an SCM-registered app_name.

Switch the consumption command to enumerate from `dashboard.applicationsOverview` (added in `@cdot65/prisma-airs-sdk` 0.12.0), which is the canonical apps-list source for the dashboard and is what the SCM AI Applications view itself uses. The dashboard's bucket `id` is the registered `customer_appId` UUID, and its `name` is the scan-payload value; both flow through directly to the per-app `dashboard.application` and `dashboard.applicationViolationBreakdown` calls.

Concrete impact on a tenant we tested: 5 customer-apps entries -> 20 dashboard buckets surfaced. A single customer-app `5e16929a-...` had 8 distinct buckets (`chatbot`, `Claude Code`, `LiteLLM`, `Portkey`, `kong-airs-demo`, ...) all with real, distinct `token_stats`. The CLI was previously showing only the first and dropping seven.

Changes:

- `getCustomerAppConsumption(name)` now looks up `(appId, appName)` from `dashboard.applicationsOverview` instead of `customer_apps.list`. Single-app mode now accepts the names shown in the SCM AI Applications view (the literal scan-payload `app_name`), which previously failed when those differed from the SCM-registered name.
- `runtime customer-apps consumption` (no args) enumerates from `applicationsOverview` and emits one record per dashboard bucket. Surfaces every bucket the dashboard tracks, not just one per customer-app.
- New service method `listConsumptionApps(opts?)` exposes the dashboard's enumeration as `ConsumptionAppListEntry[]`.
- Improved "not found" error: lists the first 5 available names + total count + explanation of the scan-payload-vs-SCM-name distinction.
- 5 new unit tests + 4 updated tests cover the new lookup path and the multiple-buckets-per-customer-app data model.

Closes #240. Requires `@cdot65/prisma-airs-sdk` 0.12.0 (cdot65/prisma-airs-sdk#177).
