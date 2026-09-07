---
title: Daily Environment Report
---

# Daily environment report

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

- [Download the example HTML dashboard](@site/static/examples/runtime-daily-report.htm)
- [Download the example Markdown report](pathname:///examples/runtime-daily-report.md)

The sample has 1,300 application-bucket sessions and 51 violating sessions, plus an explicit
scan-log visibility gap. It demonstrates how a useful report can coexist with incomplete evidence.

## What the report tells you

| Section | Evidence and interpretation |
| --- | --- |
| At a glance | API-reported sessions, violating sessions, a defensible violation rate, and source completeness |
| What needs attention | Observed violating sessions and scan failures; explicit inactive profiles, allow-on-timeout settings, disabled storage masking; collection gaps |
| Daily application activity | Rolling one-day application buckets, ranked by violating sessions |
| Current security profiles | Latest returned revision by name, active state, timeout action, storage masking, modification timestamp |
| Registered application inventory | Current names, environment, cloud, model, and counts of key associations—not key values |
| Collected scan-log observations | Action/verdict counts and tokens from collected, timestamp-eligible log entries, when available |
| Evidence and collection coverage | SDK method, window, page count, record count, and complete/partial/unavailable status for every source |

Findings are review guidance, **not an invented numeric health score**. A violating session is
not proof of a successful attack or of successful blocking. Inactive profiles and allow-on-timeout
settings may be deliberate. Missing policy settings remain **Unknown**, never silently “disabled.”

## Authentication and safety

Only Management API credentials are required: `PANW_MGMT_CLIENT_ID`, `PANW_MGMT_CLIENT_SECRET`,
and `PANW_MGMT_TSG_ID`, or `mgmtClientId`, `mgmtClientSecret`, and `mgmtTsgId` in the existing
`~/.prisma-airs/config.json`. Normal environment-over-file precedence applies. The config is read,
not written; it can remain on a read-only mount. Endpoint overrides use the normal Management API
configuration. Scanner API keys are not needed.

The report projects only approved fields. It omits raw API responses, prompts, responses, user
identities/IP addresses, API keys, auth codes, tenant IDs, and raw exception text. Strings are
escaped for HTML/Markdown, and HTML uses a restrictive content-security policy with hash-pinned
inline scripts and styles. **Names and configuration metadata can still be confidential.** Review
the artifact before distributing it. Debug logging (`--debug` or enabled `PANW_AI_SEC_DEBUG`) is
refused for this command so raw scan-log content is not persisted alongside a shareable report.

## Windows, limits, and incomplete evidence

The default is a **rolling last 24 hours**, not “yesterday at midnight.” The report displays an
anchored UTC start/end and collection timestamps. The application API evaluates a relative
one-day window separately on each request, so pages can drift during collection and telemetry
can arrive late. Historical date selection and previous-day comparisons are not claimed.

The collector uses four independent SDK reads, 100 requested records per page, and a default
10-page budget per source. `--max-pages` accepts 1–100. It stops repeated/non-advancing pagination,
deduplicates identities, and preserves earlier pages when a later request fails. A full page
without pagination metadata is followed by another page to avoid silent truncation. The SDK
enforces request deadlines (`PANW_AI_SEC_TIMEOUT_MS`, default 60,000 milliseconds); automatic
retries are disabled for this bounded report collection.

The application's daily session data is kept separate from scan entries and current inventory.
Scan logs use `24 hours`; only entries with usable received timestamps inside the anchored
half-open interval `[start, end)` contribute to the log summary. No tenant-wide totals are inferred
from a capped collection. Missing or inconsistent counters suppress the affected rate/total.

The SDK's per-application consumption and detector/severity breakdown endpoints require
7/30/60-day windows; their figures are **not** presented as daily metrics. Session time buckets
are not summed into a trend because the observed bucket semantics need further verification.
Configuration snapshots are not configuration-change audit logs, and this report is not an
availability SLA or compliance attestation.

| Exit | Meaning |
| --- | --- |
| 0 | Deliverable generated, possibly with clearly disclosed partial/unavailable sources or findings |
| 1 | Generation failed, every source was unavailable, or `--strict` found an incomplete source; inspect any generated artifact |
| 2 | Invalid options, unsupported output format, or debug logging requested |

General terminal output preferences (such as `PANW_CLI_OUTPUT=json`) do not change the default
HTML deliverable. Explicit global `--output` is honored if it is `html` or `markdown`; an explicit
command-local `--output` wins.

## Validated live output

Read-only validation on **2026-09-07 at 15:00 UTC** used the existing config without changing it.
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
CLI-generated dashboard files still use `.html`.

The public samples above remain synthetic; the live environment artifacts stay private.
