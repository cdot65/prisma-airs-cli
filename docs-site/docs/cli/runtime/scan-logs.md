---
sidebar_label: scan-logs (broken)
---

# runtime scan-logs

:::danger Broken legacy retrieval — under refactor

The SDK's `ScanLogsClient` / `client.scanLogs.query()` and the legacy
`/v1/mgmt/scanlogs` workflow must currently be considered broken. Live checks returned
HTTP 400 for rejected queries and empty HTTP 200 bodies/objects for other queries despite
known session activity. Neither response establishes that there were zero scans or threats.

CLI **5.0.0** deliberately makes `airs runtime scan-logs query` exit **1** with migration
guidance, without calling this endpoint or emitting a misleading empty list. Its old flags
remain recognizable, but changing a Scanner API key or time unit is not a verified repair.

:::

## Use the verified SCM session workflow

```bash
# One page, or a bounded walk with completeness checks
airs runtime sessions list --interval 1 --unit day --output json
airs runtime sessions list --interval 1 --unit day --all --output json

# Human-readable daily report in your current directory
airs runtime report
```

These commands use Management OAuth and the alternate SCM dashboard host. They retrieve
sessions, not the old scan-log response shape. Migrate consumers explicitly; do not treat
session counts as scan-action or detector-event counts.

See [session commands](sessions.md), [dashboard commands](dashboard.md), and the
[daily report guide](../../runtime/daily-report.md). Session → transaction → stored-content
drill-down is supported; sensitive content is fetched only when explicitly requested.
