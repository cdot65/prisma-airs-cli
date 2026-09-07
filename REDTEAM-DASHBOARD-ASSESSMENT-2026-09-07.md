# Red Team CLI environment dashboard — implementation and release assessment

## CLI 5.1.0 release candidate

The user authorized publication after the local implementation. SDK 0.27.0 is now published
through trusted npm publishing, with green CI and Node 18/20/22/24 tests. The CLI pins the
registry SDK and its integrity in the lockfile; the active dependency is no longer a local link.

Against that registry package, all 1,406 CLI tests and coverage gates pass, with no known
production dependency vulnerabilities. The built CLI passes the four Red Team E2E tests again
at 23:13 UTC. An independently packed and freshly installed consumer has identical contents,
all expected exports/commands, SDK 0.27.0, and another four passing live tests at 23:14 UTC.
The eleven native consumer checks pass with the existing explicit font configuration; the
first native attempt correctly exposed this container's absent `/etc/fonts` configuration,
which was not suppressed or fixed by changing product code. Docs include actual registry-SDK
CLI stdout and the Docusaurus build passes. No credential configuration bytes changed.

Evidence directories: `artifacts/redteam-dashboard/2026-09-07T23-13-15.337Z/` and
`artifacts/redteam-dashboard/2026-09-07T23-14-40.478Z/`. The initial local-only assessment and
release prerequisites below are retained as history; publication is now authorized.

## Delivered scope

`airs redteam dashboard` collects seven read-only SDK feeds and produces default HTML or
Markdown in the current working directory. The existing `redteam report <jobId>` is unchanged.
The collection/evidence model, rendering and command wiring are separate; private atomic,
no-clobber file publication reuses the existing report writer. Both renderers share one
aggregate-only model. HTML uses inline assets, hash-based CSP and no network dependencies.

Collection paginates targets, scans and adapters with a bounded per-source budget. Repeated
identities, changed totals, missing arrays, premature empty pages and failed pages cannot be
reported as a complete empty inventory. Strict mode delivers partial evidence before exiting 1.
All-unavailable evidence exits 1; findings alone do not change the successful collection exit code.
Debug logging is rejected before traffic collection. No target/adapter config, credentials,
names/IDs, scripts, prompts or raw upstream errors are included in report artifacts.

Only scan creation timestamps are filtered locally to the preceding 24 hours. Statistics and
quota windows remain independent server snapshots. Zero unlimited quota allocation is not
exhaustion; not-online broker channels are review items, not asserted outages. Risk categories
are reported as supplied, without inventing a numerical environment health score.

## Verification

- All 1,406 unit/integration tests pass, including 31 new collector/renderer/command tests.
- Source type checking, lint (seven existing unrelated test warnings), ESM/declaration build,
  Docusaurus type checking and production build pass.
- Four built-CLI live E2E tests pass: private default-CWD HTML with seven complete sources;
  Markdown file and exact stdout; strict partial-page output/exit status; overwrite and debug guards.
- Live artifacts: `artifacts/redteam-dashboard/2026-09-07T22-35-01.917Z/`.
- Read-only configuration bytes are unchanged. No scans are submitted or configuration mutated.
- Chromium passes six deliverable checks: rendering, priority filter, print restoration,
  mobile layout, no-JavaScript reading and no remote requests/page errors. Mobile screenshot
  inspected visually; no horizontal overflow at 390px.
- The Docusaurus command page includes exact captured CLI stdout. Source comparison and rendered
  browser comparison both pass; the local/unreleased notice is visible.
- The initial browser comparison omitted Docusaurus visual-line separators. Comparing its
  actual rendered code lines passes at 22:40:01 UTC without altering the captured output.
  TypeDoc now uses an explicit repository base path so local SDK linking does not churn
  unrelated generated reference paths. The production build passes after this correction.
- Credential scanning of source, tests, built package/docs and private artifacts finds zero leaks.

Observed live data: 8 targets, 19 dashboard-statistics scans, 23 inventory scans (19 completed,
4 aborted), 3 active adapters, 1/7 channels online. Zero scans in the inventory were created
within the local 24-hour window; this does not imply zero attacks, completions or other activity.
Risk profile includes 2 critical and 1 high entries. Static quota is unlimited, dynamic usage
0/50 and custom usage 5/50. These are dated observations, not fixed test fixture expectations.

## Historical local-only readiness boundary

Assessment: **9/10 for this bounded local integration**, not the original full-product objective
or release readiness. The source, built CLI and actual deliverables are ready for local review.
The remaining release dependency is explicit: publish the SDK's additive GET quota method,
then update the CLI's pinned SDK version/lockfile, retest and release with authorization.

For local testing, `node_modules/@cdot65/prisma-airs-sdk` links to the SDK checkout. The original
published dependency link is preserved as `node_modules/@cdot65/prisma-airs-sdk-published-backup`.
Package manifests and lockfiles are unchanged. Reinstalling dependencies restores SDK 0.26.0,
which reports quota unavailable (no POST fallback). Neither global CLI installation nor remote
repositories were changed, and no package versions, commits, pushes or publications were made.

Run from any desired output directory:

```bash
node /home/cdot/development/cdot65/prisma-airs-cli/dist/cli/index.js redteam dashboard --strict
node /home/cdot/development/cdot65/prisma-airs-cli/dist/cli/index.js redteam dashboard --output markdown --strict
```
