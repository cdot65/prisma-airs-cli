# Report naming release — September 8, 2026

## Scope

CLI 5.3.0 standardizes HTML/Markdown environment deliverables on `report` for Runtime,
Red Team and AI Gateway. Red Team and AI Gateway retain `dashboard` compatibility aliases.
`redteam report <jobId>` still displays individual scan results. Runtime's `dashboard`
group remains a separate API-query interface. Source modules, command help, README,
Docusaurus references and tests use the canonical terminology. Documentation URLs are
preserved; Gateway and Red Team example captures are refreshed from npm-installed 5.3.0.

SDK 0.28.0 is unchanged, already published and synchronized with its GitHub remote.
No empty SDK release was created for a CLI-only change.

## Pre-publication verification

- CLI unit/integration tests: **1,479 passed**, 105 files.
- Coverage gates: **97.04%** statements/lines, **96.04%** functions, **90.14%** branches.
  As configured in this repository, these percentages exclude CLI command modules;
  the command regressions are included in the passing test count.
- Release-contract tests: **14 passed**.
- Production dependency audit: **no known vulnerabilities**.
- Type checking, formatting, CLI build and Docusaurus build passed.
- Lint: no errors; seven existing warnings in the unrelated runtime command tests.
- Independent tarball install: seven payload files identical to the built checkout,
  29 library exports, SDK 0.28.0, canonical and compatibility help verified.
- Native DLP consumer: **11/11 passed**, including all five file formats.
- Installed-tarball live E2E: **16/16 passed** across two runs (nine Red Team/Gateway tests
  followed by seven Runtime tests). AI Gateway's 25 sources and Red Team's
  seven sources completed. Both suites verified unchanged credential-file bytes,
  private CWD HTML/Markdown delivery, stdout delivery, strict partial evidence,
  and overwrite/debug guards. No inference, scans or configuration mutations were run.

Private evidence is under `artifacts/aigateway-dashboard/2026-09-08T15-27-10.771Z/`
and `artifacts/redteam-dashboard/2026-09-08T15-27-10.770Z/`.

Tarball SHA-256: `e294ac30b0d352f1b833bbc2057a23cfeec51fa7dc0f81fe572e18255d84c327`.
Release commit: `5a2a2a18e755be7dac32c12888acba4fdfc36abf`.

## Publication

The release commit and annotated `v5.3.0` tag are pushed to both CLI remotes (GitHub
and the configured origin). GitHub release:
[v5.3.0](https://github.com/cdot65/prisma-airs-cli/releases/tag/v5.3.0).

- [CI](https://github.com/cdot65/prisma-airs-cli/actions/runs/34244979541): passed,
  including native consumers on Node 20.17, 22.13 and 24.
- [Docs deployment](https://github.com/cdot65/prisma-airs-cli/actions/runs/34245957414):
  passed with the refreshed npm-installed examples; Chromium verified HTTP 200, exact
  captured stdout on both example pages, and zero page errors. The corresponding
  [documentation CI](https://github.com/cdot65/prisma-airs-cli/actions/runs/34245957412) passed.
- [npm publication](https://github.com/cdot65/prisma-airs-cli/actions/runs/34245011557):
  passed, including signed provenance. Registry propagation completed: `latest` is 5.3.0.
- [Container publication](https://github.com/cdot65/prisma-airs-cli/actions/runs/34245008946):
  in progress; architecture checks and alias promotion pending.

## Registry-installed acceptance

The user-prefix global `airs` now reports **5.3.0**. All seven installed payload files match
the independently packed release tarball; the package retains 29 library exports and pins
published SDK **0.28.0**. Native DLP: **11/11 passed**, without runtime warnings.

All **16/16 live E2E tests passed** against the npm-installed CLI: Runtime **7/7**, Red Team
**4/4**, AI Gateway **5/5**. Credentials stayed byte-for-byte unchanged. Runtime additionally
confirmed HTTP 200 for the replacement session query and body-free private debug logs in CWD.
Strict evidence gaps remain visible, and existing output files cannot be overwritten.

Private npm acceptance evidence: `artifacts/aigateway-dashboard/2026-09-08T15-34-40.849Z/`,
`artifacts/redteam-dashboard/2026-09-08T15-34-40.850Z/`, and
`artifacts/runtime-report/2026-09-08T15-34-40.914Z/`. Docusaurus Gateway and Red Team example pages contain the actual captured
Markdown stdout, not synthesized SDK output. Gateway capture is protected by a byte-exact
SHA-256 regression assertion.

The first registry-install attempts returned ETARGET during npm's explicitly reported processing
period. No republish or dependency bypass was used; normal registry installation succeeded
after propagation. Container architecture checks and alias promotion remain pending above.
