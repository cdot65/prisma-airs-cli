# AI Gateway CLI dashboard — release assessment

Scope: finish the CLI consumption of the 25 supplied SCM Gateway reads, test actual
deliverables and atomic commands, update Docusaurus from CLI output, and publish SDK/CLI.
This is not a claim of full coverage of every upstream AI Gateway/Portkey API.

## Delivered implementation

- SDK 0.28.0 adds five typed reads and validated zero-based transaction pagination;
  twenty supplied reads already existed. It is published on npm and its docs are deployed.
- CLI 5.2.0 pins registry SDK 0.28.0; no local SDK link or vendored dependency is required.
- `airs aigateway dashboard --workspace dev` creates private, self-contained HTML by default
  or Markdown. Default deliverables are unique files in CWD, not the read-only config directory.
- Every telemetry request uses one fixed window. All 25 feeds have explicit source status;
  bounded pagination rejects duplicate identities, changing totals and out-of-scope rows.
- Only aggregate evidence is retained. Debug traffic capture is refused; atomic organisation
  settings and analytics filter data are redacted. Errors never copy upstream bodies into reports.
- Shared rendering retains Runtime/Red Team compatibility, HTML escaping, inline CSS/JS,
  hash-based CSP, offline/no-JavaScript operation, priority filtering and print support.
- Five new atomic reads and `logs list --current-page` are exposed, tested, and documented.

## Evidence

| Gate | Result |
| --- | --- |
| SDK tests | 11,619 passed; 99.72% lines/statements, 100% functions, 96.74% branches |
| SDK supplied captures | 25/25 schema fixtures passed |
| SDK built live reads | 27/27 passed at 2026-09-08T01:10:17.506Z; config unchanged |
| CLI regression suite | 1,462 tests passed across 104 files |
| CLI measured coverage | 97.09% lines/statements, 96.04% functions, 90.18% branches; measured before two additional docs-only tests |
| New Gateway report collector | 100% lines/functions, 92.59% branches |
| Release contract tests | 14 passed |
| Production dependency audit | No known vulnerabilities |
| Types, formatting, lint | Passed; seven pre-existing lint warnings, no new warnings |
| Built CLI live workflow | 5/5 passed; 25/25 sources complete, 1,431 unique transactions over 29 pages |
| Independently installed packed CLI | 5/5 live workflows passed at 2026-09-08T01:26:47Z; config unchanged |
| Package consumer | All seven payload files identical to local build and tarball; five distribution files; 29 library exports; help/SDK pin verified |
| Native DLP compatibility | 11/11 consumer cases passed, including all five formats; zero runtime warnings using the existing explicit host font configuration |
| Chromium report | Desktop/mobile, priority filters, print, no-JavaScript fallback passed; no external requests or page errors |
| Docusaurus | Production build passed; Chromium verified command page and exact rendered CLI stdout; no page errors |
| Credential scan | 752 changed/build/report files checked; zero matching credential values or pasted bearer tokens |

Live tests cover HTML and Markdown defaults, private file permissions, stdout, strict incomplete
delivery, no-clobber behavior, pre-I/O debug refusal, all five new atomic reads and two distinct
transaction pages. The full collector independently retrieves every transaction page. No inference,
key rotation or configuration changes were performed; config hashes were checked unchanged.

The final npm-installed CLI stdout is retained privately at
`artifacts/aigateway-dashboard/2026-09-08T01-34-17.967Z/actual-stdout.md` and reproduced exactly in
`docs-site/docs/cli/aigateway/dashboard-examples.md`. SHA-256:
`a2cbfdfc6ad9b62f24e2868677a65f23b296fecdf4197f33876d0a38c4104f4d`.
A regression test protects that exact capture; these are not reconstructed SDK examples.

## Review, corrections and limitations

The implementation review caught two improvements before release: retaining named Red Team
renderer wrappers to preserve generated API reference links, and explicitly flagging server
error analytics that classify HTTP 200 responses as errors. Both were corrected and retested.
An E2E assertion originally expected `***`; the SDK correctly returned `[REDACTED]`. The
assertion was corrected and the entire live workflow rerun successfully.

In the fixed September 7 window, 74 HTTP 500 errors and 71 AIRS blocks were observed, with two
HTTP 200 responses classified as errors. These are historical environment findings, not failed
SDK/CLI retrieval integrations. Server metric semantics are preserved rather than silently altered.

Remaining limitations: undocumented endpoints can change; inventory continuation is not verified;
catalog availability is not enabled protection; independent reads are not atomic; no invented
numerical environment health score or full upstream API-equivalence claim is made.

Self-assessment: **9/10 for this bounded feature**. The implementation and local, consumer and
published-release evidence satisfy the feature scope. Publication and independent review are below.

## Final publication and independent review

Independent reviewer: **9.1/10**, no unresolved release-blocking findings. The reviewer independently
ran 91 targeted tests, checked both live workflow logs, SDK contracts, privacy/pagination behavior,
and the diff. One P3 copy/paste issue (doubled shell continuation backslashes in the reproduction
command) was fixed and regression-tested. The review explicitly excludes publication status.

SDK **0.28.0** and CLI **5.2.0** are published with npm `latest` tags. CLI release commit
`1648cee3a9bb329bb4cb1600e804a1c0d67c2789` and tag `v5.2.0` are pushed to GitHub and the
configured origin mirror. The global `airs` command resolves to the npm installation and reports
5.2.0. Its payload is byte-identical to the verified local tarball/build (all seven files).

The actual npm-installed CLI passed **5/5 live workflows** on September 8 at 01:34 UTC,
again collecting all 25 feeds and all 1,431 transactions with the read-only credential file
unchanged. Native DLP compatibility again passed 11/11 cases. The latest npm-installed stdout
replaces the earlier build capture in the Docusaurus examples page and its checksum regression.

GitHub CI, npm publication and docs deployment succeeded. CI includes consumer runtime checks
on Node 20.17.0, 22.13.0 and 24. The public Docusaurus page was verified in Chromium with exact
CLI stdout and no page errors. Both linux/amd64 and linux/arm64 container verification jobs pass;
shared-alias promotion also completed successfully for this verified image.
