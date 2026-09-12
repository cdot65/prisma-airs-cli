---
title: Daily Environment Report
---

# Daily environment report

Available in CLI **5.0.0**, backed by SDK **0.26.0**. The exported report model is schema **2**.

:::danger Legacy scan-logs is broken and under refactor

`ScanLogsClient` and `airs runtime scan-logs query` are not reliable historical retrieval paths.
Empty HTTP 200 responses are not evidence of zero activity. CLI 5.0.0 disables that command
with an explicit exit-1 migration message. This report now uses the verified SCM
[session workflow](../cli/runtime/sessions.md); it does not call the broken route.

:::

`airs runtime report` retrieves **AI Runtime Security** activity and configuration through the
SDK and produces a deliverable for a human review. It does not submit scans, change profiles,
rotate keys, or modify your credential file.

```bash
# Self-contained HTML in your current working directory (default)
airs runtime report

# Markdown in your current working directory
airs runtime report --output markdown

# Choose a new destination and a title
airs runtime report --title "Production daily review" --output-file ./production-daily.html

# Explicitly stream the deliverable for a pipeline
airs runtime report --output markdown --output-file - > daily.md

# Generate the report, but return exit 1 if any evidence source is incomplete
airs runtime report --strict --max-pages 20
```

Default files are named `airs-runtime-report-<UTC timestamp>-<unique suffix>.html` or `.md`.
The destination is **the directory you run the command from**, not `~/.prisma-airs/`.
Use `--output-file -` for stdout; progress goes to stderr and `--quiet` suppresses progress.
Existing files and symlinks are never overwritten. File output is atomically published with
mode `0600`; an explicitly named parent directory must already exist. Redirection with `>`
uses your shell's own overwrite and permission rules instead.

The HTML includes all CSS and JavaScript inline. There are no CDNs, fonts to download,
browser API calls, or external assets. Open it directly from disk, search application buckets,
filter findings by priority, or use **Print / save PDF**. Printing includes all rows, even ones
hidden by filters. The full report remains readable with JavaScript disabled.

## Example deliverables

These download samples are **synthetic fixtures**, not customer data:

- [Download the example HTML report](@site/static/examples/runtime-daily-report.htm)
- [Download the example Markdown report](pathname:///examples/runtime-daily-report.md)

The sample has 1,300 application-bucket sessions and 51 violating sessions, plus an explicit
configuration review findings. Session inventory, chart measurements and detector events are
displayed separately, not forced into application-bucket totals.

## What the report tells you

| Section | Evidence and interpretation |
| --- | --- |
| At a glance | API-reported sessions, violating sessions, a defensible violation rate, and source completeness |
| What needs attention | Observed violating sessions and scan failures; explicit inactive profiles, allow-on-timeout settings, disabled storage masking; collection gaps |
| Daily application activity | Rolling one-day application buckets, ranked by violating sessions |
| Current security profiles | Latest returned revision by name, active state, timeout action, storage masking, modification timestamp |
| Registered application inventory | Current names, environment, cloud, model, and counts of key associations—not key values |
| Collected session observations | Status counts from timestamp-eligible session inventory entries; no scan text |
| Daily session chart | Independently measured sessions, violating sessions, and detector violations by time bucket |
| Top applications by detector violations | API-ranked subset with detector counts; not a complete inventory |
| Daily detector severity trend | Critical/high/medium/low detector events from the one-day trend endpoint |
| Evidence and collection coverage | SDK method, window, page count, record count, and complete/partial/unavailable status for every source |

Findings are review guidance, **not an invented numeric health score**. A violating session is
not proof of a successful attack or of successful blocking. Inactive profiles and allow-on-timeout
settings may be deliberate. Missing policy settings remain **Unknown**, never silently “disabled.”

## Authentication and safety

Only Management API credentials are required: `PANW_MGMT_CLIENT_ID`, `PANW_MGMT_CLIENT_SECRET`,
and `PANW_MGMT_TSG_ID`, or `mgmtClientId`, `mgmtClientSecret`, and `mgmtTsgId` in the existing
`~/.prisma-airs/config.json`. Normal environment-over-file precedence applies. The config is read,
not written; it can remain on a read-only mount. Endpoint overrides use the normal Management API
configuration. `mgmtDashboardEndpoint` / `PANW_MGMT_DASHBOARD_ENDPOINT` defaults to
`https://api.apps.paloaltonetworks.com/aisec`, independently of `mgmtEndpoint` for profiles and
registered applications. Scanner API keys are not needed.

The report projects only approved fields. It omits raw API responses, prompts, responses, user
identities/IP addresses, API keys, auth codes, tenant IDs, and raw exception text. Strings are
escaped for HTML/Markdown, and HTML uses a restrictive content-security policy with hash-pinned
inline scripts and styles. **Names and configuration metadata can still be confidential.** Review
the artifact before distributing it. Debug logging (`--debug` or enabled `PANW_AI_SEC_DEBUG`) is
refused for this command so traffic content is not persisted alongside a shareable report.
Session transaction and stored-content APIs are never fetched by this report.

## Windows, limits, and incomplete evidence

The default is a **rolling last 24 hours**, not “yesterday at midnight.” The report displays an
anchored UTC start/end and collection timestamps. The application API evaluates a relative
one-day window separately on each request, so pages can drift during collection and telemetry
can arrive late. Historical date selection and previous-day comparisons are not claimed.

The collector uses **seven independent sources**: application overview, profiles, registered
applications, session inventory, session chart, top application violations, and severity trend.
It requests 25 sessions per page and 100 records per page for application/configuration
inventories, with a default **40-page budget per source**. `--max-pages` accepts 1–100.
It stops repeated/non-advancing pagination,
deduplicates identities, and preserves earlier pages when a later request fails. A full page
without pagination metadata is followed by another page to avoid silent truncation. The SDK
enforces request deadlines (`PANW_AI_SEC_TIMEOUT_MS`, default 60,000 milliseconds); automatic
retries are disabled for this bounded report collection.

The application's daily session data is kept separate from session inventory and current
configuration. Session inventory uses `1 day`; only entries with usable `last_session_activity`
timestamps inside the anchored half-open interval `[start, end)` contribute to its summary.
Session pagination also checks stable totals, exact offsets, early empty pages and overlapping
composite identities. No tenant-wide totals are inferred
from a capped collection. Missing or inconsistent counters suppress the affected rate/total.

The SDK's per-application consumption and detector/severity breakdown endpoints require
7/30/60-day windows; their figures are **not** presented as daily metrics. The new one-day
session chart and detector trend are independent sources. Their counters are retained, and
chart/inventory disagreements are flagged for review without inventing a cause or correcting
the data. Application overview buckets are not used to reconstruct their top-level totals.
Configuration snapshots are not configuration-change audit logs, and this report is not an
availability SLA or compliance attestation.

| Exit | Meaning |
| --- | --- |
| 0 | Deliverable generated, possibly with clearly disclosed partial/unavailable sources or findings |
| 1 | Generation failed, every source was unavailable, or `--strict` found an incomplete source; inspect any generated artifact |
| 2 | Invalid options, unsupported output format, or debug logging requested |

General terminal output preferences (such as the tenant's `defaultOutput: json`) do not change the default
HTML deliverable. Explicit global `--output` is honored if it is `html` or `markdown`; an explicit
command-local `--output` wins.

## Validated live output

**Latest installed verification — CLI 5.0.1 / SDK 0.26.0, 2026-09-07 at 19:32 UTC:**
the complete live suite passed **8/8** against the published package installed as `airs`.
All seven sources were complete: **6 application buckets, 19 profiles, 17 registered
applications, 822 sessions across 33 pages, 10 chart buckets, 3 ranked applications, and
10 severity-trend buckets**. Application activity reported **822 sessions / 79 violating**.
The endpoint workflow completed 15 checks, including rejected `week` and successful
seven-day YAML retrieval. HTML/Markdown, default CWD delivery, strict completeness status,
no-clobber protection, and private body-free session debug checks passed. Credential bytes
were unchanged. These figures are dated observations, not fixed expected totals.

One earlier 5.0.1 packed-candidate attempt passed 7/8 and failed during full session
pagination; the failure's cause was not established. A traced rerun returned 822 unique
sessions across 33 stable HTTP-200 pages, and the unchanged full candidate suite then passed
8/8 before publication. The failed capture remains in the release assessment; it is not
relabeled as successful. Broader generic-error diagnostics remain a separate known issue.

**CLI 5.0.0 / SDK 0.26.0 candidate, 2026-09-07 at 17:39 UTC:** all seven read-only report
workflow tests pass. Every source is complete: **6 daily application buckets, 19 profiles,
17 registered applications, 677 session entries across 28 pages, 10 session-chart buckets,
3 ranked applications, and 10 severity-trend buckets**. Application activity reports
**677 sessions and 81 violating sessions**. These are timestamped observations, not constants
or a guarantee that separately fetched counters always agree.

Validation covers direct SDK collection; built CLI HTML/Markdown; strict completeness status;
atomic no-clobber files; default CWD deliverables; the replacement session debug command
(HTTP 200, body-free private log in CWD); and credential/raw-field exclusion. The credential
file's hash is unchanged. Private live artifacts are not included in these downloads.

:::warning Earlier CLI 5.0.0 authentication failures

At **18:15 UTC**, the complete eight-test live suite was rerun through the user's installed,
published CLI **5.0.0** with registry SDK **0.26.0**: **2 passed / 6 failed**. Most fresh OAuth
attempts timed out before returning HTTP. The strict-mode request did succeed, returning
all seven sources complete and **709 session entries across 29 pages**; no-clobber also passed.
Authentication is therefore intermittent, not completely unavailable. The protected
content check also could not run to completion because the earlier generation test did not
produce its Markdown input. Independent credential scans still pass. Curl and
published SDK 0.25.0 reproduced the same failure, while the dashboard API remained reachable
with unauthenticated HTTP 401. The earlier successful tests are retained as dated evidence,
not relabeled as a passing latest run. No credentials, DNS or tenant configuration were changed.

Publication itself is verified: registry and installed payloads match all seven package files
and 23 exports; both pass eleven native checks. All remote CI jobs pass. The published site
and both linked downloads pass nine browser checks. A separate unavailable-source check
confirms a diagnostic report is written and strict mode exits 1; unknown activity is not zero.

:::

## Migrating from CLI 4.5

CLI 5.0 is a major version because the exported `RuntimeDailyReport` changes from schema 1
to schema 2. Replace `report.logs` / `ReportLogSummary` with `report.sessions` /
`ReportSessionSummary`; use `report.dailyTelemetry` for chart/ranking/severity evidence.
Injected `RuntimeReportClient` implementations must provide the five dashboard reads in the
new interface, plus profiles/customer-app inventory. HTML remains the default and Markdown,
private CWD destinations, no-clobber behavior and strict completeness exits remain supported.

## Historical CLI 4.5 validation

Read-only validation on **2026-09-07 at 15:19 UTC** used the existing config without changing it,
through both the published-registry installation and the user's installed CLI **4.5.0**. Neither
final run needed the host DNS bootstrap used during earlier investigations.
The SDK collection returned **7 daily application buckets, 19 profiles, and 17 registered
applications**. Across the collected daily buckets it reported **705 sessions and 91 violating
sessions**. These are a point-in-time observation, not expected constants for a future run.

The scan-log endpoint returned an empty object rather than a record array. The report marked
it **unavailable**, produced both deliverables, and returned exit 1 for the strict run. It did
not convert that response into “zero threats.” All seven live workflow checks passed, including
default CWD output and the debug-path regression. The 128-hour debug query reached the API and
received HTTP 400, with its private log successfully written in CWD; API acceptance is not claimed.
Real Chromium
checks passed for offline rendering, filtering, printing, mobile layout, JavaScript-disabled
reading, hostile metadata, live HTML opening, and zero external requests/browser errors. A ninth
check verified the documentation guide and exact bytes of both linked example downloads. The HTML
download uses `.htm` (standard HTML) to avoid the preview server's `.html` clean-URL redirect bug;
CLI-generated report files still use `.html`.

The public samples above remain synthetic; the live environment artifacts stay private.
