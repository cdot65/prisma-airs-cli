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
