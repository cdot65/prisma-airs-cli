---
sidebar_label: sessions
---

# runtime sessions

CLI **5.0.0** / SDK **0.26.0** provide verified SCM session retrieval, replacing the workflow
served by the [broken legacy scan-logs command](scan-logs.md). The schemas differ intentionally.
Authentication and endpoint overrides are described under [dashboard](dashboard.md).

```bash
airs runtime sessions chart --interval 1 --unit day --output json
airs runtime sessions list --interval 1 --unit day --limit 25 --offset 0 --output json
airs runtime sessions list --interval 7 --unit days --output yaml
airs runtime sessions list --all --max 10000 --output json

airs runtime sessions get --session-id SESSION_ID \
  --app-id APPLICATION_ID --app-name 'Exact name' --interval 30 --unit days

airs runtime sessions transaction --session-id SESSION_ID \
  --app-id APPLICATION_ID --app-name 'Exact name' \
  --scan-id SCAN_ID --scan-sub-req-id 0 --interval 30 --unit days
```

Chart/list default to **1 day**; get/transaction default to **30 days**. List/get accept
`--limit` (25) and `--offset` (0). All accept `--output pretty|json|yaml`; pretty is readable
JSON. List JSON/YAML is a **bare array**; the other commands preserve their response envelopes.
List without `--all` is one page, not a complete export. Get paginates session actions separately.

Starting in CLI **5.0.1**, supported units are `hour`, `hours`, `day`, and `days`. For one week, use
`--interval 7 --unit days`, not `--interval 1 --unit week`. Unsupported units are rejected
with exit code **2** and a usage hint before configuration loading, authentication, or API
requests. Units are not silently converted. These undocumented APIs may impose additional
window limits; accepting a unit does not guarantee every interval is supported by the server.

`--all` verifies offsets, page limits, stable totals and unique
`(application ID, application name, session ID)` identities. It advances by actual returned
item count. `--max 0` removes the item cap, but the 100-page cap remains. A capped result is
explicitly partial and exits 1; inconsistent pagination fails without a claimed complete export.
Exit 2 denotes invalid options; authentication/API/output errors exit 1. Rolling windows are
not snapshot exports, so changing traffic can require a later rerun.

## Explicit stored-content access

Use the action's `scan_id` and `scan_sub_req_id` from `sessions get`. Zero is valid. Choose
exactly one content destination:

```bash
# Explicitly display sensitive content
airs runtime sessions scan-content --scan-id SCAN_ID --scan-sub-req-id 0 --show-content

# Or write a new private file (0600); never overwrites an existing file or symlink
airs runtime sessions scan-content --scan-id SCAN_ID --scan-sub-req-id 0 \
  --output-file ./stored-scan.json
```

Stored prompts/responses and transaction attributes can be confidential. No other session
command or daily report fetches stored content automatically. The response field is
`sub_scan_req_id`, distinct from the query's `scan_sub_req_id`. Null content is valid and is
not replaced with empty text. Avoid redirecting stdout to a public artifact. Debug logs
contain request/status metadata, but omit dashboard and scan-content bodies.

## Live verification

The read-only release workflow walks session pages, then uses an actual returned identity
to fetch session actions, a transaction and stored content. It verifies all eleven supplied
dashboard/report routes, without saving or printing customer text. See the dated
[daily report evidence](../../runtime/daily-report.md#validated-live-output) for the separate
human-deliverable E2E results. Counts vary as the relative window advances.

The **5.0.1 unit-validation patch** passed the full eight-test live suite before release on
2026-09-07 at 19:15 UTC. Its endpoint workflow completed 15 checks, including exit 2 and a
seven-day hint for `week`, plus a successful `--interval 7 --unit days --output yaml` query.
A separate built-CLI check verified zero network calls for the rejected unit. The daily
inventory contained 775 sessions across 31 pages, with all seven report sources complete.
Credential-file integrity checks passed; these results do not replace earlier failed captures.
