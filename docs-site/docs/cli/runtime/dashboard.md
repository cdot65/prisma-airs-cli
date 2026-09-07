---
sidebar_label: dashboard
---

# runtime dashboard

CLI **5.0.0**, using SDK **0.26.0**, exposes the supplied SCM application dashboard routes.
All commands are read-only and use Management OAuth. These undocumented APIs may change;
only the supplied and live-tested windows are claimed verified.

| Command | Required identity | Default window | Response |
| --- | --- | --- | --- |
| `applications` | None | 1 day | `items`, `pagination`; `--limit 25 --offset 0` |
| `application` | `--app-id`, `--app-name` | 30 days | Application, profiles, session/token statistics |
| `application-violations` | `--app-id`, `--app-name` | 30 days | Detector/severity breakdown |
| `top-applications` | None | 1 day | `applications` ranked subset |
| `violations-trend` | None | 1 day | `violations` time buckets |
| `apps-list` | None | 30 days | `applications` identity pairs; no verified pagination |

```bash
airs runtime dashboard applications --interval 1 --unit day --output json
airs runtime dashboard apps-list --output yaml
airs runtime dashboard application --app-id APPLICATION_ID --app-name 'Exact name'
airs runtime dashboard application-violations --app-id APPLICATION_ID --app-name 'Exact name'
airs runtime dashboard top-applications --output json
airs runtime dashboard violations-trend --output json
```

All support `--interval`, `--unit`, and `--output pretty|json|yaml`. Pretty output is readable
JSON; JSON/YAML preserve the SDK response envelope. Application detail and breakdown require
7, 30, or 60 `days`; their metrics must not be labeled daily. Overview accepts the SDK's
1/7/30/60 and day/days/hour options; arbitrary combinations are not guaranteed by the service.

Keep application ID **and exact name** together: IDs can repeat across names, and names
across IDs. Preserve scaled token units. Detector violations may exceed violating sessions;
overview bucket totals may not reconcile with top-level counters.

The dashboard host defaults to `https://api.apps.paloaltonetworks.com/aisec`. Override it with
`PANW_MGMT_DASHBOARD_ENDPOINT` or config `mgmtDashboardEndpoint`; `mgmtEndpoint` remains
separate for other Management resources. Credentials are the existing `mgmtClientId`,
`mgmtClientSecret`, `mgmtTsgId`, or matching `PANW_MGMT_*` variables. No Scanner key, browser
token, Origin or Referer is needed. Debug files in CWD omit dashboard bodies; explicitly
displayed JSON/YAML may still contain confidential metadata.

See [sessions](sessions.md) for drill-down and the [daily report](../../runtime/daily-report.md)
for a shareable, allowlisted projection.
