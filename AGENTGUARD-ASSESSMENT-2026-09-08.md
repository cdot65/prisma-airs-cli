# AI Supply Chain / AgentGuard implementation assessment

Date: September 8, 2026 (UTC). Status: implementation and local verification complete; SDK 0.29.0 / CLI 5.4.0 release validation in progress. Publication results are recorded separately below once verified.

## Delivered scope

- SDK: `AgentGuardClient` with four read-only operations: scans, scan statistics, scan vulnerabilities and rule catalog. Shared Management OAuth, tenant header, distinct data/management hosts, strict query validation and forward-compatible Zod responses. Debug bodies are always omitted.
- Existing Model Security: all five supplied captures reuse existing operations and schemas; added the browser refresh hint to model/scan queries. CLI model/scan lists gained timestamp filters and scans gained model-version filtering. Corrected singular evaluation/source filter mappings to the SDK's plural array fields.
- CLI: `agentguard scans list`, `scans vulnerabilities <scanUuid>`, `stats`, `rules list`, and canonical `report`. Six terminal formats, bounded all-page retrieval, safe finding metadata by default and explicit sensitive-content opt-in.
- Reports: self-contained HTML by default, Markdown alternative, CWD destinations, atomic no-overwrite writes, mode 0600, stdout support, explicit completeness and strict-mode exit behavior. Aggregate-only 30-day evidence; no names, IDs, code or finding text.
- Docusaurus: SDK guide/API reference and CLI command guide with actual live output, pagination caveats, nullable metrics, examples and version requirements. Model Security command pages include new filters. AI Gateway category labels now expand nested pages without navigating to another sidebar.

## Verification results

| Check | Result |
| --- | --- |
| Supplied response contracts | 9/9 validate |
| Fresh OAuth + live SDK requests | All 9 requests pass |
| Complete live acceptance runner | 26/26 pass on final rerun |
| SDK automated tests | 11,638/11,638, 153 files |
| CLI automated tests | 1,522/1,522, 107 files (including sidebar regression) |
| SDK coverage | 99.72% lines/statements, 100% functions, 96.79% branches |
| New AgentGuard SDK client/models | 100% statements, lines, functions and branches |
| SDK build, typecheck, tooling typecheck, lint, example gate | Pass |
| CLI build, typecheck, lint | Pass; 7 pre-existing test lint warnings remain |
| Both Docusaurus production builds | Pass; generated guides contain actual-output examples |
| HTML browser acceptance | Desktop/mobile, filter, print, no-JavaScript pass; 0 external requests, 0 page errors |
| Secrets file | Read-only; before/after hashes match |
| Changed/new file secret scan | No configured secret values or JWTs found |

The CLI suite emits an existing Fontconfig warning in image-generation tests, which pass. A parallel SDK/CLI build initially raced declaration-file generation; the final CLI build was rerun after the SDK completed and passed. No successful release or CI deployment is claimed from these local checks.

The vault session was created and indexed. Vault-wide validation still reports unrelated unregistered HTML files and legacy migration/link errors; those were not changed or represented as a passing product check.

## Live findings

At the final acceptance run around 17:25 UTC:

- Model catalog: 7 models; model scans: 91 total; security groups: 6. Selected model-version lookup and its filtered scan query passed.
- AgentGuard: 24 historical scans retrieved with `--all --limit 3`; 13 scans within the report's explicit 30-day window, over two pages.
- Rules: all 7 retrieved with `--all --limit 3`; 6 catalog defaults BLOCKING, 1 ALLOWING. These are not effective tenant policy settings.
- Statistics: 2 unique skills, 14 vulnerabilities, top category SECRET_EXPOSURE with 7 findings. Percentage changes were null and display as unknown.
- Report outcomes: 8 SKILL/ALLOWED, 2 SKILL/BLOCKED, 3 AGENT/PENDING. Three scans lack completed decisions; failed status is not converted to a successful evaluation.
- Finding metadata: 6 findings retrieved for a discovered scan, without content exposure.

Statistics, historical inventory and bounded report inventory use different windows and identities. They are not interchangeable totals.

## Corrections prompted by live E2E

The first acceptance run passed 22/26 checks. Later pages—not present in the provided sample—contained failed scans with null summaries/durations and historical scans with null evaluation outcomes. The initial schema rejected those records and strict reports correctly failed as incomplete. Schemas and null-preserving CLI projections were corrected, regression-tested, and the full workflow rerun successfully.

The rules API reports page length in `pagination.total_items`. A three-row page initially appeared complete despite four remaining catalog rules. Direct offset probes established the behavior. Rules now advance to a short/empty page with budgets and duplicate guards; the live runner compares paginated identities with the complete catalog response to prevent a false success. Scan pagination retains global-total checks.

## Local artifacts and reproduction

Final valid report artifacts are under `artifacts/agentguard-e2e-mFFBpM/`:

- `airs-agentguard-report-2026-09-08T17-25-19.630Z-04360a9e.html`
- `airs-agentguard-report-2026-09-08T17-25-20.915Z-77d73a3e.md`

These are private local evidence, ignored by git. Earlier partial and passing runs are retained separately, not silently replaced. Automated-test JSON evidence is in SDK `artifacts/agentguard-tests.json` and CLI `artifacts/agentguard-e2e-PBncPD/unit-results.json`. Browser verification used the earlier successful HTML artifact with identical report structure/data.

The original implementation was tested with a local SDK link. Release validation replaces that link with the exact registry dependency, SDK 0.29.0. To reproduce from the released CLI checkout, install dependencies and run:

```bash
pnpm run build
AIRS_BROWSER_CAPTURE_PATH=/var/tmp/t3.txt node scripts/e2e-agentguard.mjs
node dist/cli/index.js agentguard report --strict
```

The capture variable is optional and only validates response bodies. Browser tokens are never executed, copied to code, or used for authentication.

## Release gate and self-assessment

Self-assessment: **9/10 for the captured read-only implementation and local verification**. This is not a claim of production release readiness or full undocumented API coverage. There is no formal AgentGuard OpenAPI spec in this input, so no 99% OpenAPI-coverage claim is made.

Release requirements: publish SDK 0.29.0 with these exports, update the CLI's exact SDK pin and lockfile, rerun acceptance against the registry-installed dependency, then publish CLI 5.4.0 through the established release workflow. SDK 0.28.0 / CLI 5.3.0 do not include this feature.

The navigation fix passed a browser check on both the Docs landing page and the CLI Reference workflow page: category expansion and collapse retain the URL and the top-level sidebar items. Both documentation production builds and SDK documentation typechecking pass. The original browser assertion compared all sidebar text, including newly revealed children; it was corrected to compare top-level navigation instead.

### Registry SDK release validation

- SDK 0.29.0 was published through trusted npm publishing from `cb6badbb2d7313fa139ce52d03e901d354748ed9`. Its CI, Node 18/20/22/24 tests, publication and documentation deployment all passed.
- Installed SDK 0.29.0 from npm into the CLI with an exact manifest/lockfile pin; verified the dependency resolves into pnpm's registry package store, not a checkout. Both ESM and CommonJS consumers expose `AgentGuardClient` and `SDK_VERSION === '0.29.0'`.
- At 18:39 UTC, fresh OAuth acceptance against the registry SDK passed **26/26**. Actual statistics remain 2 unique skills / 14 vulnerabilities; historical scan pagination returns 24 records and rule pagination returns all 7. Private report artifacts: `artifacts/agentguard-e2e-pDgoGL/`.
- CLI release build and typecheck pass with the registry dependency. Publication and installed-CLI verification follow this gate.

Unverified/out of scope: scan submission/uploads, policy mutation, additional statistics periods, vulnerability request pagination, and a separate Model Security report. None was inferred from GET captures or presented as implemented.
