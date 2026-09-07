# AI Runtime Security daily report — implementation assessment

Date: 2026-09-07. Scope: the user's new daily environment dashboard and follow-up requirement
to put debug logs and deliverables in the invoking working directory. This is independent of
the previous multi-product OpenAPI coverage goal.

## Design and SDK capability review

Implemented `airs runtime report` in the existing CLI; no second CLI or authentication stack.
The report layer uses the published SDK 0.25.0 and a narrow read-only client interface. Its
collector/analysis, versioned allowlisted model, HTML/Markdown renderers and private file publisher
are separate library modules under `src/reports/`, exported for reuse by future product reports.

| SDK capability | Reporting decision |
| --- | --- |
| `dashboard.applicationsOverview` | Daily activity source with `timeInterval: 1, timeUnit: day`; retain distinct ID/name buckets and paginate |
| `profiles.list` | Current latest-profile inventory with defensive per-name revision collapse; inspect explicit policy settings only |
| `customerApps.list` | Current registered inventory; never expose returned key/auth-code data |
| `scanLogs.query` | Read-only POST, bounded pagination/token handling; aggregate timestamp-eligible action/verdict entries only when a record array is present |
| Per-app consumption and detector breakdown | Excluded from daily KPIs because supported windows are 7/30/60 days |
| Historical snapshots, baseline comparisons, uptime | Not established by these reads; do not manufacture a trend, change audit, SLA or health score |

Independent source reads run concurrently; pagination stays sequential within each source.
The default budget is 10 pages of 100 requested rows per source (maximum configurable budget:
100 pages). Repeated offsets/tokens, overlaps, exhausted budgets and later-page failures never
silently turn a partial set into complete evidence. Requests use the SDK's deadlines and zero
automatic retries. No custom HTTP client, active scan, infrastructure mutation or new dependency.

## Delivery and privacy

- Default: unique `airs-runtime-report-<UTC timestamp>-<suffix>.html` in CWD; Markdown uses `.md`.
- `--output-file <new-path>` overrides the destination; `--output-file -` explicitly streams.
- Artifact files are mode 0600 and atomically published without replacing existing paths/symlinks.
- `--debug` now writes a unique private `debug-api-<timestamp>-<suffix>.jsonl` in CWD. It neither
  truncates earlier files nor prunes the user's deliverables. Initialization errors are friendly.
- OAuth form secrets and auth codes are redacted; non-JSON/inference debug bodies are omitted.
  General debug content is still confidential. Report commands refuse debug logging.
- HTML has inline CSS/JS, no external resources, hash-pinned CSP, escaped metadata, keyboard
  controls, mobile tables, no-JS readability and print styles that reveal filtered rows.
- Markdown carries the same evidence and recommendations with markup/HTML injection escaping.
- Raw prompts/responses, user identities/IPs, tenant IDs, credentials, auth codes, raw errors and
  SDK passthrough fields never enter the report model. Names/configuration metadata remain private.
- Default generated artifacts are ignored by Git and source formatting. Synthetic docs examples
  are intentionally separate. No live HTML or raw debug log is committed to public documentation.

## Validation evidence

- Full suite: 1,326 passing tests in 97 files. Reporting library: 100% lines/statements/functions,
  97.94% branches. Overall measured library coverage: 96.56% lines/statements, 95.73% functions,
  89.42% branches. CLI code is excluded by the existing coverage configuration, so its public
  command tests and browser checks are separate evidence, not implied by those percentages.
- Seven live read-only workflow checks passed at 15:00 UTC: SDK sources, built CLI HTML/Markdown,
  strict-mode artifact/exit behavior, no-clobber behavior, default CWD deliverables, debug-path
  regression and credential/raw-field exclusion. `~/.prisma-airs/config.json` remained byte-identical.
- Live source snapshot: 7 application buckets, 19 profiles, 17 registered applications; the
  earlier 14:50 UTC capture reported 705 bucket sessions and 91 violating sessions. These are
  observed values, not fixed expected outputs.
- The scan-log daily response was an empty object, so that source was **unavailable**, not empty.
  Strict mode correctly wrote the report and returned 1. Normal report generation returned 0.
- The user's exact 128-hour debug query reached the backend and received **HTTP 400**. Its private
  CWD log was created successfully. This verifies the filesystem fix, not acceptance of that API
  interval. An initial E2E assumption of HTTP 200 failed; the final check explicitly verifies and
  records the observed HTTP 400 without relabeling it as API success.
- Eight real Chromium checks passed at 14:53 UTC: rendering/CSP, application and finding filters,
  printing/PDF, mobile overflow, no-JS reading, hostile metadata, live artifact opening, and zero
  external requests/browser errors. Documentation-linked artifacts are also checked before delivery.
- Production dependency audit: no known vulnerabilities. Lint/typecheck/build/release-policy and
  Docusaurus checks are rerun after the final packaging update.

Private evidence: `artifacts/runtime-report/<capture-time>/validation.json`, the generated
HTML/Markdown and CWD debug regression logs, plus `artifacts/runtime-report/browser-validation.json`.
The first documentation build caught static HTML/Markdown links being treated as document routes;
the links now use the supported asset/static mechanisms. Broken-link gates were not disabled.

## Self-review and remaining boundaries

The implementation meets the first-product reporting design: reusable SDK collection, honest
evidence boundaries, actionable human findings, safe portable deliverables and CWD defaults.
It does not claim complete scan-log visibility, daily detector severity, uptime, historical
configuration diffs or unique tenant-wide session totals. Those need additional trustworthy
source capabilities before being introduced. Optional future work: persisted daily baselines,
verified bucket-series semantics, tenant-specific review policies and another product collector.

Candidate 4.5.0 was versioned through Changesets, npm-packed, installed in an isolated consumer,
and byte-compared with the source build: all seven packaged files and 23 library exports match;
version, inference, telemetry and report help checks pass. The initial pnpm-packed manifest differed
from the npm publishing manifest, so verification was repeated using the actual npm packing flow;
the equality check was not weakened. Full typecheck, lint/format, build, 14 release-policy checks,
production audit and strict documentation build pass. Seven existing lint warnings are unchanged.

Nine Chromium checks now pass, including byte-verified HTML/Markdown downloads through the built
Docusaurus guide. The `.html` sample hit an existing preview-server base-URL redirect bug; the
download now uses the equivalent `.htm` extension and passes, without changing CLI `.html` output.
A normal-host report invocation without the DNS bootstrap also succeeds, preserving the expected
scan-log gap. A credential scan checked 13,460 files with zero leaks and unchanged config.

Self-assessment for the **new first-product reporting scope: 9/10**. The remaining point is reserved
for upstream scan-log visibility and richer verified daily telemetry, not hidden or fabricated.
Release publication and installed-user verification will be recorded separately when confirmed.
