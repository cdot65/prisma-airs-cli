# Runtime dashboard release assessment — 2026-09-07

Scope: SDK 0.26.0 and CLI 5.0.0 for the eleven supplied AI Runtime Security SCM dashboard/report
routes, human daily deliverables, explicit legacy scan-log migration, documentation and release.
This is not a completion claim for the older, incomplete AI Gateway OpenAPI project.

## Implementation and compatibility

- SDK: raw plus lossless Zod-validated methods, separately scoped dashboard endpoint, shared
  service-account OAuth and tenant header, exact query/response names, valid zero sub-request
  indexes, open status/detector strings, nulls and fractional timestamps.
- CLI: six application-dashboard commands and five session commands. Full session enumeration
  validates stable totals, exact offsets/limits and composite identity. Page/item caps are
  explicit; inconsistent reads do not masquerade as complete exports.
- Reports: seven sources, independent session/detector measurements, evidence-backed findings,
  partial/unavailable states, safe counter/rate handling, current configuration versus daily
  activity, no inferred compliance score or unsupported daily token statistics.
- Private artifacts: self-contained HTML (inline CSS/JS) and Markdown in CWD, exclusive atomic
  0600 file creation, no clobber or automatic pruning. Explicit stdout remains available.
- Stored content is fetched only by explicit `scan-content --show-content|--output-file`.
  No automatic transaction or content retrieval by the report. Debug bodies are suppressed
  for dashboard/content routes; report debug is refused. No credentials or customer scan text
  are stored in these verification summaries or public documentation samples.
- CLI major version is deliberate: exported `RuntimeDailyReport` schema 2 replaces `logs` with
  `sessions` and adds `dailyTelemetry`; custom clients require the new methods. Legacy
  `scan-logs query` deliberately exits 1 with migration guidance. The SDK legacy class remains
  runtime-compatible but is deprecated and documented as broken/under refactor.

## Verification before publication

| Check | Result |
| --- | --- |
| SDK regression | 11,541 / 11,541 tests; 148 files |
| SDK coverage | 99.72% statements/lines; 100% functions; 96.72% branches |
| SDK packed consumer | Nine payload files and 127 source-map files match; 1,329 ESM/CJS exports; strict NodeNext types; dashboard export/query/validation checks pass |
| CLI regression | 1,354 / 1,354 tests; 99 files |
| CLI coverage | 96.67% statements/lines; report module 100% statements/lines/functions |
| Release policy | 14 / 14 tests |
| Native DLP regression | 11 / 11 built-consumer checks, five formats, no runtime warnings; existing process-only fontconfig used |
| Production dependency audit | Zero known vulnerabilities |
| Source/tooling checks | Formatting, lint, TypeScript and production builds pass |
| Documentation | Both TypeDoc/Docusaurus builds and typechecks pass; SDK example coverage passes |
| Real browser / linked examples | 9 / 9: offline HTML, CSP/JS, search/filter, print, mobile, JS-disabled, hostile metadata, live report, exact Docusaurus downloads; zero external report requests/errors |
| Secret scan | 13,043 files checked; zero configured-secret matches; credential file unchanged |

### Read-only live report — 17:39 UTC

All seven E2E tests pass: direct SDK collection, built CLI HTML/Markdown, strict status,
no-clobber, default CWD artifacts, replacement session debug (HTTP 200/body omitted/0600),
and credential/raw-field exclusion. All seven sources are complete:

| Source | Records | Pages |
| --- | ---: | ---: |
| Daily application activity | 6 | 1 |
| Profiles | 19 | 1 |
| Registered applications | 17 | 1 |
| Session inventory | 677 | 28 |
| Session chart | 10 | 1 |
| Top application violations | 3 | 1 |
| Daily severity trend | 10 | 1 |

Application buckets report 677 sessions / 81 violating sessions. These are time-bound
observations, not future expected values or forced reconciliation across separate queries.

### Built CLI all-route workflow — 17:47 UTC

All thirteen checks pass across the eleven supplied routes: six application reads, chart,
session page, complete inventory, session detail, transaction, explicit stored content, and
the disabled legacy command. Actual returned identities are followed; no pasted JWT or
hard-coded tenant session ID is used. The credential file is read-only and byte-for-byte unchanged.

## Issues found and handled

1. The release identity gate caught an SDK package/User-Agent version mismatch. Fixed the
   constant and repeated the full SDK suite and packed consumer; both pass.
2. Final report review caught stale schema-1 footer text. Both renderers now display the model
   version; tests cover schema 2 and hostile data in the new trend/ranking/chart surfaces.
3. Feasibility application discovery wrongly required a 30-day detail target to appear in the
   one-day overview. It now discovers within the same 30-day window.
4. Post-17:49 fresh OAuth reruns encountered connection timeouts before an HTTP response. Curl
   reproduces it; both published SDK 0.25.0 and the 0.26.0 candidate fail the same read at the
   OAuth stage. The SCM API remains reachable (unauthenticated HTTP 401). Public DNS resolvers
   agree on the OAuth address; the existing process-only fully-qualified DNS accommodation
   does not repair it. No retry, host change or infrastructure mutation is hidden in a pass.
   Earlier successful live evidence is retained. Final registry/installed live validation is
   pending service reachability; this is not yet an all-green operational sign-off.

## Reproduction

```bash
pnpm run build
pnpm run test:coverage
pnpm run test:release
pnpm run audit:prod
RUN_RUNTIME_REPORT_E2E=1 pnpm exec vitest run --config vitest.e2e.config.ts \
  tests/e2e/runtime-report.e2e.spec.ts tests/e2e/runtime-dashboard.e2e.spec.ts
```

Use `RUNTIME_REPORT_CLI_ENTRY` for an absolute installed CLI entry. Live outputs are under
gitignored `artifacts/runtime-report/` and `artifacts/runtime-dashboard/`. Browser verification
uses `.github/scripts/verify-runtime-report-browser.mjs` with an installed Chromium/puppeteer
module and optional `REPORT_HTML` / `REPORT_DOCS_URL`. Public example downloads are synthetic.

## Release status and self-assessment

SDK implementation is pushed as `5a59d523ff206e17b8f3fda7b441dba26ec36a69`. SDK 0.26.0 is
published on npm; [publication](https://github.com/cdot65/prisma-airs-sdk/actions/runs/34149891869),
[CI](https://github.com/cdot65/prisma-airs-sdk/actions/runs/34149690144),
[Node 18/20/22/24 tests](https://github.com/cdot65/prisma-airs-sdk/actions/runs/34149690202), and
[Docusaurus deployment](https://github.com/cdot65/prisma-airs-sdk/actions/runs/34149690196) pass.
The public dashboard guide contains the new version and explicit broken-legacy warning.

CLI now pins registry **0.26.0** exactly; there is no local path in package/lock metadata.
The repeat full suite passes **1,354/1,354** against that registry SDK. A fresh packed CLI
consumer verifies all **seven payload files and 23 exports**, version/help, all new command
names, and **11/11 native checks**. SDK registry checksum matches the independently packed
candidate. The latest secret scan checks 13,052 files with zero matches and unchanged credentials.

At 18:06 UTC, the installed candidate's fresh live all-route test fails at its first read
because OAuth remains unreachable. Its allowlisted report records zero completed checks;
no downstream endpoints are counted as passing on that attempt. Earlier 17:39/17:47 successes
remain separately dated. CLI push/publication and user-prefix installation are the remaining
release actions; the external authentication availability issue remains openly disclosed.

Current operational assessment: **8/10**, not complete until registry publication, CLI pin,
installed-package checks and fresh live verification are resolved. Implementation and earlier
E2E are strong; assigning a higher operational score would hide the current authentication
availability problem. The remaining iteration is explicit: publish verified packages, verify
exact installed payloads, repeat live checks, deploy docs and record actual final results here.

## Final publication evidence — 18:16 UTC

All authorized release actions are delivered:

- SDK **0.26.0** is on npm and GitHub, with public Docusaurus docs and independent fresh registry
  consumer verification. Its runtime release commit is `5a59d523ff206e17b8f3fda7b441dba26ec36a69`.
- CLI **5.0.0** is on npm and GitHub; commit `ac6d785b232019900be48df2231ff23a5c884596` and tag
  `v5.0.0` are on both configured CLI remotes. The user's existing `airs` is upgraded to 5.0.0.
- [CLI CI](https://github.com/cdot65/prisma-airs-cli/actions/runs/34150439098) passes every job,
  including native consumers on Node 20.17, 22.13 and 24. [npm publication](https://github.com/cdot65/prisma-airs-cli/actions/runs/34150548244)
  and [Docusaurus deployment](https://github.com/cdot65/prisma-airs-cli/actions/runs/34150439168) pass.
- Fresh registry and user-prefix installations independently match all **7 payload files / 23
  runtime exports** and each passes **11/11 native checks**. Production registry audit reports
  zero vulnerabilities. The installed CLI tarball SHA-256 is
  `8974fc8681bccfd8501b99910d1453ad51cb92cdbcc38545ddd08367adf10b2c`.
- [Container publication](https://github.com/cdot65/prisma-airs-cli/actions/runs/34150548389)
  passes both amd64/arm64 native verification jobs and guarded alias promotion. `5.0` and
  `latest` resolve to `sha256:bff6093d8cd99d4d372fe1029e0c335ba559388c3dbe6a37d2a86e2dca4ceebc`.
- Both documentation sites are live. The public CLI guide and exact downloadable examples
  pass **9/9** real-browser checks, with no external report requests or browser errors.
  ScanLogsClient/legacy command warnings and schema-2 migration instructions are explicit.

### Final full installed live suite — 18:15 UTC

The complete eight-test suite ran against the user's **published** CLI 5.0.0 / SDK 0.26.0:
**2 passed / 6 failed**. Strict-mode delivery and no-clobber protection passed. Inspection of
that strict artifact confirms a successful authenticated request: all seven sources are complete,
including 709 sessions across 29 pages, 6 application buckets, 19 profiles and 17 registered apps.
Authentication is intermittent rather than completely unavailable. This successful installed
report is retained as `strict.html`; it does not turn the six failed checks into passes.
Fresh SDK collection, positive HTML/Markdown delivery, positive default CWD delivery, session
debug retrieval and all-route drill-down failed because authentication timed out before HTTP.
The content-exclusion test could not finish because the earlier positive-generation test did
not produce its Markdown input; it is not falsely counted as a passing privacy E2E. Separate
credential scans remain successful, and credential-file integrity is unchanged.

Evidence is retained privately in `artifacts/runtime-report/2026-09-07T18-14-31.778Z/` and
`artifacts/runtime-dashboard/2026-09-07T18-14-31.710Z/`. Earlier successful 17:39/17:47 workflows
remain documented separately; their counts are not substituted for this latest result.

**Final operational assessment: 8/10.** The code, release, installed consumers and documentation
are delivered and verified, but current live readiness is **not** certified. Iteration fixed
the version mismatch, schema footer, discovery-window assumption, debug privacy gap and
documentation gaps. The remaining item is restoration of OAuth connectivity from this
environment, followed by the same full installed E2E suite. Safe read-only diagnostics and
the known process-only DNS alternative were exhausted; neither production configuration nor
credentials were changed. No provider outage cause or unauthorized network workaround is invented.
