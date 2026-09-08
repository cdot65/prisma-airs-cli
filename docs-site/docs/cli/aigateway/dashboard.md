---
sidebar_label: report
sidebar_position: 2
---

# aigateway report

`report` is the canonical name starting in CLI **5.3.0**. `dashboard` remains a
compatibility alias. CLI 5.2.0 uses the older spelling. This documentation URL remains stable.

The built `report` command passed all five live E2E workflows on September 8, 2026
(15:07–15:09 UTC): 25 complete sources, HTML/Markdown file and stdout delivery,
strict truncation, overwrite/debug protection, and atomic telemetry reads. The credential
file remained unchanged. The verified-output page records actual npm-installed CLI 5.3.0 stdout.

Added in CLI **5.2.0**, with the published SDK **0.28.0** pinned exactly.
Generate a read-only daily environment deliverable from all 25 supplied SCM Gateway feeds:
17 telemetry charts, analytics filter boundaries, paginated transactions, active tenant
workspaces, selected-workspace configs and service/user key inventories, organisation info,
and the guardrail evaluator catalog.

```bash
npm install --global @cdot65/prisma-airs-cli@5.3.0
airs aigateway report --workspace ws-develo-71f8d8
airs aigateway report --workspace dev --output markdown
airs aigateway report --workspace dev --strict --output-file ./gateway-health.html
airs aigateway report --workspace dev --start 2026-09-07T00:00:00Z \
  --end 2026-09-08T00:00:00Z --output markdown --output-file -
```

| Option | Default | Behavior |
| --- | --- | --- |
| `--workspace` | Required | Slug, UUID, or unique exact display name; resolved using the admin workspace list |
| `--days` | `1` | Positive whole rolling days; one fixed start/end pair for every telemetry read |
| `--start`, `--end` | Unset | Supply both timezone-qualified ISO-8601 timestamps; cannot combine with explicit `--days` |
| `--output` | `html` | `html` or `markdown`; independent of the configured terminal output preference |
| `--output-file` | New timestamped file in CWD | Never overwrites; `-` streams only the deliverable to stdout |
| `--title` | AI Gateway environment report | 1–240 characters |
| `--max-pages` | `40` | 1–100 transaction pages, 50 records each; budget exhaustion is partial evidence |
| `--strict` | Off | Exit 1 after writing if any source is incomplete |

Files are atomically published with mode **0600**. Existing paths and symlinks are refused.
HTML includes inline CSS and JavaScript, a hash-based CSP, priority filtering, print support,
and a no-JavaScript fallback. It has no external assets or network calls. Status messages go
to stderr. Debug logging is refused before collection to prevent sensitive traffic persistence.
The existing Management OAuth configuration is read-only; data/admin/token endpoint overrides
are respected. No inference, key rotation, or environment changes occur.

Exit **0** means delivery succeeded, not that the environment is risk-free. Exit **1** means
operational failure, no complete telemetry sources, or strict incomplete evidence; the diagnostic
report is still delivered when possible. Invalid options exit **2** before authentication.

## Interpreting the report

The report separates telemetry-window activity from current configuration and tenant capabilities.
It displays request/error/token/cost/latency/cache/feedback/retry metrics, error status categories,
an error timeline, inventory counts, and transaction pagination evidence.
HTTP 429/5xx errors require attention; other errors and AIRS blocks request review without
automatically declaring an outage. Catalog availability does not establish enabled protection.

Costs remain **cents**, latency is **milliseconds**, feedback can be negative, and null is unknown.
Independent server aggregates and pages are not an atomic snapshot. Duplicate rows, changing totals,
wrong-workspace/out-of-window transactions, quota flags, unavailable feeds and truncated inventories
are explicitly incomplete, never a healthy zero. Inventory continuation is not invented.

Deliverables retain only allowlisted aggregate counters, UTC buckets and HTTP status codes.
They exclude API keys, user identities, names/IDs, request URLs, prompts, raw logs, metadata values,
configuration bodies and organisation settings. Treat even aggregate reports as confidential.

## Atomic reads added in this release

```bash
airs aigateway telemetry error-category-trends --workspace ws-develo-71f8d8 --days 1 --output json
airs aigateway telemetry grouped-errors --workspace ws-develo-71f8d8 --days 1 --output yaml
airs aigateway telemetry filter-boundaries --workspace ws-develo-71f8d8 --days 1 --output json
airs aigateway guardrails catalog --output json
airs aigateway organisations info --tsg-id YOUR_TSG_ID --output json
airs aigateway telemetry logs list --workspace ws-develo-71f8d8 --current-page 0 --output json
airs aigateway telemetry logs list --workspace ws-develo-71f8d8 --current-page 1 --output json
```

`--current-page` is zero-based; `--page-size` remains 50 by default. `offset`, `skip`, and `page`
are not aliases for the verified SCM query. Keep the same explicit start/end window across pages.
Atomic log reads deliberately return transaction data: redirect only to a private destination.
`filter-boundaries` redacts the entire data subtree (it can contain key IDs and metadata), and
`organisations info` redacts settings. The report consumes those SDK responses in memory and
publishes only safe aggregate counts.

## Live examples

The companion [verified CLI output](./dashboard-examples.md) records actual npm-installed package E2E
output and checks. It is not SDK output presented as a CLI transcript.
