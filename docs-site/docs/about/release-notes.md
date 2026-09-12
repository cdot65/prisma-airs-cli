# Release Notes

## v6.1.0 (2026-09-12) — Provider slugs, credential inputs, and self-hosted endpoints for integrations

- `airs aigateway integrations providers` lists the provider catalog, and `integrations create`
  accepts `--ai-provider <slug-or-uuid>` (`x-ai`, `open-ai`, …) next to `--ai-provider-id`.
- Credentials no longer have to sit in `argv`: `--key-file <path>` and `--key-stdin` join
  `--secret-mappings`; inline `--key` still works but warns. A create with no credential now
  stops with a usage error naming the remedies instead of the gateway's opaque `400 AB01`.
- `--base-url <url>` and repeatable `--header name=value` on `create` and `update` write the
  live-verified self-hosted endpoint shape (`provider_auth_type: apiKey`, `custom_host`,
  `custom_headers`), so an in-cluster OpenAI-compatible server needs no hand-built JSON.
- Pins SDK 0.33.0 (`integrations.catalog()`, `resolveProviderId()`, `customHostConfiguration()`).

## v6.0.0 (2026-09-12) — Tenant files are the only configuration source

- **Breaking:** `airs config` is removed and no environment variable configures the CLI any
  more. `dotenv` and `.env` loading are gone, `PRISMA_AIRS_CONFIG_PATH` is ignored, and the
  `~/.prisma-airs/config.json` "default" tenant no longer exists. Register a tenant with
  `airs tenant create <name>` (or `--config <path>` for an existing file) and select it with
  `airs tenant switch <name>`; with no selection every API command stops with
  `No tenant selected` and lists the registered names. Only `PRISMA_AIRS_TENANTS_PATH` and
  `XDG_STATE_HOME` (registry location) and the SDK diagnostics `PANW_AI_SEC_DEBUG`,
  `PANW_AI_SEC_DEBUG_BODY`, `PANW_AI_SEC_TIMEOUT_MS` are still honored.
- `airs tenant` now covers everything `airs config` did: `get <name> <key>`,
  `unset <name> <key>` (credentials cannot be cleared), and `path [name]` join `create`,
  `switch`, `set`, `list`, `read`, and `delete`. Deleting the selected tenant clears the
  selection instead of being refused.
- **Breaking:** one SCM OAuth credential set and one token endpoint per tenant. Management,
  DLP, Red Team, Model Security, AgentGuard, AI Gateway, and SCM IAM all authenticate with
  `mgmtClientId` / `mgmtClientSecret` / `mgmtTsgId` through `mgmtTokenEndpoint`. The
  per-product token endpoint keys (`redTeamTokenEndpoint`, `modelSecTokenEndpoint`,
  `agentGuardTokenEndpoint`, `aiGwTokenEndpoint`) are retired and ignored, and the SDK's
  per-product credential variables are never consulted because the CLI now passes every
  credential and endpoint explicitly. Product base-URL overrides remain file-only keys with
  SDK defaults.
- `airs doctor` is tenant-first: it fails clearly when no tenant is selected, validates the
  tenant file against its pinned TSG, warns about retired keys in the file and about
  `PANW_*` / `PRISMA_AIRS_CONFIG_PATH` variables still set in the shell (names only), treats
  a missing scanner key as skipped, and phrases every remedy as `airs tenant set`.
- Every product base URL now defaults to `api.apps.paloaltonetworks.com` (AISEC management,
  Red Team, Model Security, AgentGuard, AI Gateway, IAM); only DLP stays on
  `api.dlp.paloaltonetworks.com`. The values come from the SDK constants, so this lands with
  SDK 0.32.0, which this release pins.
- Live e2e scripts and specs resolve the operator's selected tenant through the registry
  (`AIRS_E2E_TENANT=<name>` picks another registered one) instead of a config-path variable.

## v5.11.1 (2026-09-11) — SDK 0.31.1

- Pin SDK 0.31.1, which fixes `generateWorkspaceScopeName()` on Node 18. The CLI requires
  Node 20 or newer, so 5.11.0 was unaffected; this keeps the dependency current.

## v5.11.0 (2026-09-11) — Scope-first AI Gateway workspace provisioning

- `airs aigateway workspaces create` now provisions a workspace the way Strata Cloud
  Manager's UI does (captured 2026-09-11): create the SCM IAM scope, create the
  workspace with that `scope_name`, then PUT the scope back with the new workspace slug
  bound as a resource. That last step is what grants data-plane access. A bare create
  against a scope that does not exist yet is the `400 AB01` seen on September 6.
- `--scope-name` is optional and defaults to SCM's `ws_<name>_<suffix>` convention;
  `--existing-scope` binds a scope created earlier and preserves its other bindings.
  Partial failures are reported with the created slug and scope, never hidden.
- New `airs aigateway scopes {list, get, create, bind, delete}` expose each step on its
  own and list unbound scopes. `list`/`get` were verified live; `create`/`bind` send the
  captured SCM bodies; `delete` is not live-verified. `PANW_IAM_ENDPOINT` / `iamEndpoint`
  override the IAM base URL.
- Fix `--output json` being ignored on `workspaces create` and `update` (the root
  program's own `--output` consumed the flag).
- Pin SDK 0.31.0 (`gw.iamScopes`, `gw.workspaces.provision()`).

See [workspaces](../cli/aigateway/workspaces.md) and the
[workflow cheat sheet](../cli/aigateway/workflows.md#create-a-workspace).

## v5.9.1 (2026-09-11) — DLP dictionary and profile migration remediation

- Pin published SDK 0.30.1 for sanitized RFC 7807 field diagnostics. Dictionary API
  errors exit 1; invalid local input exits 2 without leaking payload fragments.
- Correct dictionary region examples to the live-verified SCM label `United States`.
  CSV restores preserve the first keyword by rebuilding the header; TXT remains
  newline-delimited. Five dictionaries, including a UI-created 19-keyword dictionary,
  restored prod → dev with exact re-GET verification.
- Sort DLP inventories and validate IDs/counts across pages, including the live
  50-record cap and snake_case pagination. Incomplete catalogs fail closed.
- Build profile flag inputs from verified pattern identities and the actual rule
  tree shape. Refuse unsupported basic writes and names over the observed 32-character
  limit before API writes.
- Harden transfer preflight, typed reference compatibility, and create verification;
  retain confirmed created IDs when verification cannot complete.
- Preserve version-1 envelopes and explicit mapping/whole-profile skips. See the
  [live acceptance transcript](../runtime/dlp/transfer.md)
  for the exact live findings and separately identified mocked failure cases.

## v5.9.0 (2026-09-11) — Tenant-first DLP listings and live-verified transfer

- `airs runtime dlp {patterns, profiles, dictionaries} list` now shows only
  tenant-created records by default; predefined (PANW-shipped) catalog content is
  hidden until requested with `--include-predefined`.
- Backup refuses profiles referencing retired patterns instead of resurrecting
  archived configuration in the destination, and refuses basic profiles whose rule
  content the API does not export; both are excluded with reasons under
  `--skip-unsupported`.
- Restore classifies patterns without exported detection content (UI copies of
  predefined patterns) as unresolvable, with `--pattern-map` and `--skip-unresolved`
  as explicit remedies, and tolerates the server's `supported_confidence_levels`
  normalization — reported, never silently accepted.
- Post-create verification failures now name the created record id, since the write
  landed even though it did not verify.
- The pattern and profile transfer paths are now live-verified end to end: a
  complete cross-tenant migration ran with creates, read-back verification, reuse,
  verify-resume, and explicit skips. The dictionary create path remains
  live-unproven — a live probe matrix indicates a tenant-level restriction on custom
  dictionaries, and restores involving dictionaries fail safe. See
  [live acceptance](../runtime/dlp/transfer.md) in the guide.

## v5.8.0 (2026-09-10) — DLP backup and restore

- Add `airs runtime dlp backup` and `airs runtime dlp restore`: staged transfer of
  custom DLP dictionaries (with keyword payloads), data patterns, and data profiles
  between tenants through a private, size-capped, no-clobber file.
- Restore stages dictionaries → patterns → profiles, remapping profile expression-tree
  references (id, name, version) to destination identities, with a full plan validated
  before any write and destination state re-checked after confirmation.
- Filter predefined (PANW-shipped) resources out of backups; referenced predefined
  patterns and dictionaries are embedded as resolve-only references and matched in the
  destination catalog by name, never created.
- Enforce dependency closure at the exact referenced revision: stale pattern-version
  pins, multi-profile rules, and direct EDM dataset references fail the export, or are
  excluded with reasons under `--skip-unsupported`.
- Require explicit `--pattern-map` bindings for tenant-bound detection techniques (EDM,
  fingerprints, trained models, linked dictionaries); no lossy fallback exists.
- Verify every create by re-reading the record, including keyword round-trip and
  lifecycle state; report server-added fields, partial completion, and exit 1 on any
  incomplete restore. `--on-conflict` supports `error`, read-only `verify`, and `skip`;
  no update or delete is ever issued pending live verification of the DLP write path.
- Reviewed under the AIRS Transfer Contract agent gate (9/10 PASS); see the
  [DLP backup and restore guide](../runtime/dlp/transfer.md) for the end-to-end
  prod-to-dev example and review evidence.
- Also ships the previously merged read-only Red Team environment dashboard with
  private HTML/Markdown deliverables, seven SDK data sources, explicit completeness,
  and verified actual command-output examples.

## v5.7.0 (2026-09-09) — verified Runtime migration and Basic DLP fallback

- Recognize built-in Basic DLP as portable across tenants, without a custom DLP mapping.
- Add explicit `--on-missing-dlp basic`, with mapping priority, per-profile protection-loss
  warnings and structured fallback details. Default behavior remains fail-closed.
- Preserve actions, masking and disabled detection; reject unsafe fallback configurations.
- Accept the live API's absent-to-null normalization for the optional `database-security`
  field during read-back verification; other policy changes still fail verification.
- Verify observed server-added severity and empty default URL-category fields only when
  absent from the source, and report accepted additions in `serverDefaults`.
- Include category-level toxicity confidence defaults, requiring matching parent detector
  and category identity; explicit severities and action changes still fail verification.
- Cover observed topic-guardrails, blocked-topic and blocked contextual-grounding severity
  defaults. Add a real full-tenant migration acceptance runner and
  [copy-and-paste workflow](../runtime/profile-migration-workflow.md).
- Add `--on-conflict verify` for partial-restore recovery: verify matching destination
  profiles without writes, reject mismatches before mutation and create only missing profiles.
- Render human restore summaries as one resource per row instead of oversized JSON cells.
- Use SDK 0.30.0's typed Runtime policy extensions. See [live MVP acceptance](../runtime/dlp-fallback-validation.md);
  recursive DLP profile/pattern/dictionary migration is not included.

## v5.6.0 (2026-09-08) — guided tenant configuration

- `airs tenant create <name>` now prompts for TSG ID, OAuth client ID, and a hidden
  client secret, one field at a time. Existing `--config <path>` registration remains supported.
- `airs tenant set <name> <key> [value]` changes individual settings without switching
  tenants. Omit the value to prompt; credentials require hidden input or `--stdin`.
- Automation can create a config using `--tsg-id`, `--client-id`, and
  `--client-secret-stdin`. No secret argument is needed.
- Private configs, atomic updates, per-config locks, read-only checks, pinned TSG
  identity, and cancellation without partial registrations protect tenant configuration.
- The [tenant guide](../cli/tenant.md) includes actual terminal output. Validation
  includes 1,619 regression tests, built-CLI local OAuth, and cross-tenant migration tests.

## v5.5.0 (2026-09-08) — tenant selection and Runtime profile migration

- Add `airs tenant create`, `switch`, `list`, `read`, and `delete`, registering existing
  read-only config files by path. Store the selection separately without copying secrets.
- Pin each registration's TSG identity, redact credential values fully, reject mixed
  tenant/environment authentication, and retain legacy `default` behavior.
- Add `airs runtime profiles backup` and `restore`: private JSON/YAML files, exact topic
  dependencies, dry-run plans, destination assertions, explicit conflict handling,
  destination ID rewriting, and read-back verification.
- Require explicit mapping to existing destination DLP data profiles for cross-tenant
  migration; never silently reuse source IDs or update shared topic definitions.
- Preserve existing files and report partial restore failures without automatic rollback.

The [migration guide](../runtime/profile-transfer.md) contains live backup output and
the verified synthetic restore workflow. Two isolated OAuth tenants exercise the actual
CLI's cross-tenant path; live cloud verification uses the single configured tenant.
SDK remains **0.29.0**; its existing Management APIs provide the required operations.

## v5.4.1 (2026-09-08) — AgentGuard reports and navigation repair

Version 5.4.0 was withheld from npm after the release gate found a test-only dependency
on the separately installed Docusaurus toolchain. Version 5.4.1 isolates that sidebar
regression test; no release tag was rewritten.

- Add `airs agentguard scans list`, `scans vulnerabilities`, `stats`, `rules list`
  and `report`, backed by SDK 0.29.0 and the existing Management OAuth credentials.
- Deliver private, no-clobber HTML or Markdown reports in the current directory. Reports
  contain aggregates only; raw finding content requires explicit opt-in.
- Handle nullable scan outcomes and page-count rule metadata without reporting false
  zero activity or silently truncating the rule catalog.
- Expose captured Model Security time-window and model-version filters.
- Make the AI Gateway sidebar category expand its nested pages instead of navigating
  to the workflow page and switching sidebars. The workflow remains its first child.

The [AgentGuard command guide](../cli/agentguard/index.md) includes actual live output
from the 26-check read-only acceptance workflow and documents data limitations.

## v5.3.0 (2026-09-08) — consistent report commands

- Use `airs runtime report`, `airs redteam report`, and `airs aigateway report --workspace dev`
  for HTML (default) or Markdown environment deliverables.
- Keep `dashboard` as a compatibility alias for Red Team and AI Gateway. Runtime's
  `dashboard` API-query group is unchanged.
- Preserve `airs redteam report <jobId>` for individual scan results. Scan-only and
  environment-only options cannot be mixed.

Verified against the npm-installed CLI: **16/16 live E2E tests** across Runtime, Red Team
and AI Gateway; **11/11 native DLP checks**; exact seven-file package payload match. The full
unit/integration suite passes **1,479 tests**. Credentials remain unchanged. Updated
[Gateway examples](../cli/aigateway/dashboard-examples.md) and
[Red Team examples](../cli/redteam/dashboard.md) contain actual 5.3.0 stdout.
SDK remains the published **0.28.0**; no SDK change was required.

## v5.1.0 (2026-09-07) — Red Team environment dashboard

- Add `airs redteam dashboard` with HTML (default) and Markdown deliverables. Preserve
  `redteam report <jobId>` for individual scans.
- Pin SDK 0.27.0 and collect seven read-only Red Team feeds, including GET quota. Explicit
  pagination budgets and source completeness prevent missing evidence from becoming zero activity.
- Keep server-default statistics separate from the local 24-hour scan-creation window. Do not
  infer broker outages or quota exhaustion from incomplete or unlimited counters.
- Produce private, atomic, no-clobber files in CWD. Refuse debug logging and exclude raw
  configuration, credentials, scripts, prompts and scan identities from deliverables.
- Provide inline CSS/JS, hash-based CSP, offline/mobile/no-JS rendering, priority filters and
  print support. Include actual CLI stdout in the [command examples](../cli/redteam/dashboard.md).

The local implementation passed 1,406 tests and four built-CLI live E2E tests; release validation
also exercises the registry SDK and package consumer rather than relying on a local dependency link.

## v5.0.1 (2026-09-07)

Runtime dashboard/session commands reject unsupported time units before config loading,
debug-file creation, authentication, or API requests. `--unit week` now exits **2** with a
clear hint to use `--interval 7 --unit days`, instead of a generic HTTP 400 error. Session
queries retain `hour`, `hours`, `day`, and `days`; application-specific commands retain
their narrower existing constraints. Units are not silently converted.

Command help and [session examples](../cli/runtime/sessions.md) describe the accepted units.
Validation: **1,375** regression tests, **45** focused command tests, and **8/8** live E2E tests
pass. The endpoint workflow includes 15 checks, including rejected week and successful
seven-day YAML retrieval. An instrumented built-CLI rejection probe confirms **zero fetch
calls**, even with invalid configuration and debug enabled. The SDK remains pinned to 0.26.0.

The [npm publication](https://github.com/cdot65/prisma-airs-cli/actions/runs/34155639222),
[release CI](https://github.com/cdot65/prisma-airs-cli/actions/runs/34155384917),
[docs deployment](https://github.com/cdot65/prisma-airs-cli/actions/runs/34155384857), and
[container publication](https://github.com/cdot65/prisma-airs-cli/actions/runs/34155638754)
pass, including both native architectures and guarded container aliases. Fresh registry
and global CLI installations match the tested payload and each pass eleven native checks.
The installed published CLI passes the full live suite **8/8** at 19:32 UTC, including both
the rejected week and working seven-day YAML commands. An earlier candidate pagination
failure and subsequent passing reruns are retained in the [dated evidence](../runtime/daily-report.md#validated-live-output).

## v5.0.0 (2026-09-07)

- Pin SDK 0.26.0. Add `runtime dashboard` application summaries and `runtime sessions` inventory, chart, session, transaction and explicit stored-content access across all eleven supplied SCM routes. Reuse Management OAuth with a separately configurable dashboard host; no browser token or Scanner key is required.
- Replace the broken legacy scan-log source in daily reports with verified session inventory, chart, ranking and daily severity data. All seven sources expose completeness, window and collection evidence. Keep session counts distinct from detector events and surface inconsistencies instead of forcing reconciliation.
- **Breaking library change:** report schema 2 replaces `logs` / `ReportLogSummary` with `sessions` / `ReportSessionSummary` and adds `dailyTelemetry`. Custom report clients now need five dashboard methods plus profile/application inventories. The default per-source page budget is 40; session pages request 25 rows.
- **Breaking command behavior:** `runtime scan-logs query` now exits 1 with an explicit broken/refactor warning. Use `runtime sessions list`; this is a workflow migration, not a compatible wire-schema substitution.
- Keep stored content opt-in (`--show-content` or private no-clobber `--output-file`). Suppress dashboard/content bodies in CLI debug logs as well as SDK logs. Reports never fetch transaction attributes or stored content.
- Preserve self-contained HTML/Markdown, private CWD output, no-clobber publication and offline interactions. Update references, migration instructions and synthetic downloads; keep historical E2E failures clearly dated.

See [daily report validation and migration](../runtime/daily-report.md),
[dashboard commands](../cli/runtime/dashboard.md), and [session commands](../cli/runtime/sessions.md).
The candidate passes all 1,354 regression tests against registry SDK 0.26.0, exact packed/installed
payload checks and eleven native checks. Earlier live workflows pass; a later fresh installed
run encounters an independently reproduced OAuth connection timeout. See the dated availability
warning in the guide; this is not an all-green current service-availability claim.

The [npm publication](https://github.com/cdot65/prisma-airs-cli/actions/runs/34150548244),
[CI](https://github.com/cdot65/prisma-airs-cli/actions/runs/34150439098),
[docs deployment](https://github.com/cdot65/prisma-airs-cli/actions/runs/34150439168), and
[container publication/amd64+arm64 checks](https://github.com/cdot65/prisma-airs-cli/actions/runs/34150548389)
pass. Registry and user-prefix CLI 5.0.0 match the verified seven-file payload and 23 exports,
with eleven native checks each and an unchanged credential file. Public browser/download
checks pass 9/9. The full installed live suite at 18:15 UTC reports **2 passed / 6 failed**,
blocked by OAuth connectivity and its missing-artifact consequence; it is not counted as
a passing latest E2E run.

## v4.5.0 (2026-09-07)

- Add `airs runtime report`: read-only daily AI Runtime Security activity and current configuration, with evidence-backed attention/review findings and explicit complete/partial/unavailable sources. Self-contained HTML is the default; Markdown is also supported. No numeric health score or unsupported daily severity/token metrics are invented.
- Deliver dashboards and `--debug` logs in the current working directory, not beside read-only credentials. Unique default names, private permissions, no overwriting, no automatic pruning. Reports support `--output-file -` for stdout and `--strict` for completeness-sensitive automation.
- Add offline application search, priority filtering, mobile/print layouts, CSP-pinned inline assets and hostile-metadata escaping. Raw prompts, responses, credentials, tenant/user identifiers and upstream errors are excluded from report artifacts.
- Handle debug-log initialization failures with a friendly error. Mask OAuth form secrets and auth codes; omit non-JSON debug bodies. A live 128-hour scan-log query now writes its log successfully, but the service still rejects that interval with HTTP 400.
- Keep SDK 0.25.0 pinned; no SDK runtime changes are required. Scan-log detail remains explicitly unavailable when the service returns an empty object. The new command does not change the scope or state of AI Gateway.

See the [daily report guide and downloadable examples](../runtime/daily-report.md) for the
read-only live capture, source limitations, and human review workflow. Samples published with
the documentation are synthetic; live deliverables remain private.

The [published package](https://github.com/cdot65/prisma-airs-cli/releases/tag/v4.5.0) and the
upgraded user installation each pass all seven live report/debug workflows at 15:19 UTC with
unchanged credentials. Both match all seven package files and 23 exports from the verified
build. [CI](https://github.com/cdot65/prisma-airs-cli/actions/runs/34137084360),
[npm publication](https://github.com/cdot65/prisma-airs-cli/actions/runs/34137254995),
[docs deployment](https://github.com/cdot65/prisma-airs-cli/actions/runs/34137084383), and
[container publication with both native architecture checks](https://github.com/cdot65/prisma-airs-cli/actions/runs/34137086577)
pass. Public browser/download verification passes 9/9.

## v4.4.0 (2026-09-07)

- Pin published SDK 0.25.0. Extend the nine verified analytics filters to `group-by ai_service|model|api_key|provider|status_code|users`.
- Use the SDK's dimension/column lists and semantic filter schema. Invalid dimensions, unsupported user columns, malformed CSV/JSON and unsafe or reversed numeric bounds exit 2 before client creation.
- Preserve all existing chart behavior, units and group JSON/YAML envelopes. Non-user columns remain supported; user grouping does not gain unverified column options.
- Add 95 failing-first public CLI/registry-SDK transport regressions. All 1,268 tests pass; native, installed-package and live candidate workflows are verified separately before publication.
- Keep the full-scope assessment at 5/10: direct Gateway coverage remains 138/242 (57.02%). Existing service/model/entitlement failures are not resolved by this release.

The published npm package passes 103/103 grouped-filter checks, 54/54 chart contracts, 8/8 inference checks, 3/3 empty-window checks and 11/11 native DLP checks. The upgraded user-prefix CLI passes 12/12 cross-service reads/benign scan and 11/11 native DLP checks. All 36 historical release keys are independently confirmed retired. Both installations match all seven payload files and 19 exports. The [container workflow](https://github.com/cdot65/prisma-airs-cli/actions/runs/34121350201) passes 11/11 native checks on each actual architecture before verifying minor/latest aliases. The earlier 102/103 group, 53/54 registry-chart and 10/11 DNS-affected browser attempts remain disclosed; complete subsequent suites pass without changes to validation, retries or infrastructure. See [actual grouped output](../cli/aigateway/telemetry.md#verified-grouped-filter-output) and [current inference capture](../cli/aigateway/inference.md#latest-verified-example-output).

## v4.3.1 (2026-09-07)

- Fix `runtime dlp generate --output json`, including global, config-file and environment selection. Unsupported formats and malformed or unsafe integer counts/seeds now exit 2 before generating files. Quiet pretty output preserves per-format counts.
- Upgrade production `js-yaml` to 4.3.2, `nanoid` to 5.1.16 (including the DOCX dependency), and optional `sharp` to 0.35.4. The frozen production audit reports zero known advisories; CI, npm publication and container publication now enforce that audit instead of treating functional tests as security verification.
- Correct Node engine metadata and `airs doctor` to the actual existing dependency intersection: `^20.17.0 || ^22.13.0 || >=23.5.0`. Add native CLI smoke checks at Node 20.17, 22.13 and 24; SDK Node 18 support is unchanged.
- Publish exact container version tags first, verify the digest on amd64 and arm64 without runtime network access, then serialize and guard minor/`latest` promotion against the current stable tag list. Older tagged builds cannot advance aliases owned by newer versions. Branch/prerelease dispatches are rejected; do not rerun legacy workflows from tags predating this safeguard.
- Include fontconfig/DejaVu in the minimal Alpine runtime and allow only required build inputs into the Docker context. The native smoke verifies all five DLP formats, 26 file signatures, manifest counts and structured-output/validation behavior. Local validation uses an existing process-only font configuration because this host has no system fonts.
- Keep SDK 0.24.0 pinned. The full-spec assessment remains 5/10: direct AI Gateway coverage is still 138/242 (57.02%), and the documented upstream service/model limitations are not fixed by this patch.

The [release CI](https://github.com/cdot65/prisma-airs-cli/actions/runs/34110678594) passes all seven jobs, including Node 20.17/22.13/24 native consumers. The [container workflow](https://github.com/cdot65/prisma-airs-cli/actions/runs/34110815270) passes 11/11 native checks on each architecture and verifies `4.3`/`latest` at digest `sha256:c113c58d457e0152b5f51c1cab850d771b7ddeac9ca88d8020b74442791cfadf`. Registry and user-installed packages independently match all seven payload files and 19 exports; registry production audit and 11/11 native checks pass. The exact [native output](../cli/runtime/dlp/generate.md) is captured after registry installation, not copied from a mock.

## v4.3.0 (2026-09-07)

- Add verified trace, string-metadata, status-code, API-key-ID, provider/model, total-token and cost-range filters to `aigateway telemetry requests`, `cost`, `tokens` and `latency`. Lists use OR; different filters use AND; numeric bounds are inclusive. Cost bounds are in cents, including fractional cents.
- Pin published SDK `0.24.0` and use its exported filter schema before creating a CLI client or resolving a cost workspace. Malformed input exits 2 without authenticated requests or input values in diagnostics.
- Reject partially parsed day counts such as `7junk`; resolve cost workspace UUIDs to telemetry slugs. Existing pretty-dollar and explicit JSON/YAML cents/USD fields remain compatible. The cost command retains its rolling-day window.
- These verified query options do not establish full upstream analytics equivalence. Direct gateway coverage remains 138/242 (57.02%); provider/service limitations remain in the [SDK assessment](https://cdot65.github.io/prisma-airs-sdk/developer/openapi-conformance/).
- Registry-installed checks pass 54/54 chart contracts, 8/8 inference and 3/3 empty-window checks; upgraded user-install checks pass 12/12. The separate production lockfile audit still flags `js-yaml`, `nanoid` and optional `sharp`; fresh npm resolution leaves the `sharp` advisory and its package-level propagation. This is not a clean security sign-off; dependency remediation remains a focused follow-up.

## v4.2.2 (2026-09-07)

- Pin SDK `0.23.0`, correcting empty latency response validation. `aigateway telemetry latency` preserves null period mean/percentiles in JSON and YAML instead of failing on a valid empty cohort.
- Add failing-first public-command integration checks against the actual installed SDK, with synthetic HTTP responses and no live credentials.
- The SDK also adds verified trace/metadata filters to cost, token and latency methods. This CLI release adds no filter flags. Direct gateway coverage remains 138/242 (57.02%), and existing provider/service limitations remain explicit in the [SDK assessment](https://cdot65.github.io/prisma-airs-sdk/developer/openapi-conformance/).

## v4.2.1 (2026-09-07)

- Pin SDK `0.22.0`, including its experimental caller-owned realtime transport. The CLI's supported inference commands remain `chat`, `responses` and `embeddings`; this release adds no realtime CLI command and does not promise realtime support for the prescribed model.
- Run the full lint, format, typecheck, coverage and documentation CI gates on main pushes. Publication now enforces formatting and coverage as well.
- Keep actual version-specific inference captures on the [examples page](../cli/aigateway/inference.md). SDK direct gateway coverage is 138/242 (57.02%); its realtime live test reaches HTTP 101 but fails with `invalid_model`. Existing service failures and the incomplete full OpenAPI target remain explicit in the [SDK assessment](https://cdot65.github.io/prisma-airs-sdk/developer/openapi-conformance/).

## v4.2.0 (2026-09-07)

### New

- Add `airs aigateway inference chat`, `responses` and `embeddings`, with explicit runtime endpoint/key configuration separate from SCM OAuth.
- Support streamed text and JSONL, stdout backpressure, signal cancellation and secret-safe diagnostics. Runtime requests default to zero automatic retries.
- Pin SDK `0.21.0` for the typed runtime API, expanded AIRS contracts, OAuth deadline recovery and transport hardening.

### Validation and limitations

- Pre-release CLI validation passes 1,033 tests and 8/8 live inference checks using the prescribed dev workspace and models. Actual sanitized output is included in the [inference examples](../cli/aigateway/inference.md).
- Correct DLP cleanup help: the unsupported delete command exits 2 without API traffic; a failing status-patch workflow is not presented as verified retirement.
- This release is authorized with known AI Gateway gaps. The SDK matches 137/242 upstream operations (56.61%), and experimental methods and backend failures remain documented. The complete primary SDK example run has 18 passing and 3 failing scripts; publication does not claim all-green E2E or 99% full AI Gateway coverage.
- Live gateway evidence uses the documented TLS-verified LAN path; public WAN reachability remains uncertified. See the [SDK conformance report](https://cdot65.github.io/prisma-airs-sdk/developer/openapi-conformance/).

## v4.1.2 (2026-08-31)

### Changed

- Replaced Prisma AIRS CLI branding across the README, Docusaurus navbar, home page, favicon, and
  social preview with the new shield, terminal, beam, and spectrum logo.
- Consolidated the previous icon and light/dark wordmark variants into one canonical SVG so every
  published surface uses the same artwork.

## v4.1.1 (2026-08-31)

### Fixed

- AI Gateway service and user API-key list/detail commands now redact stored key material by
  default. Use the explicit `--reveal-sensitive` flag only when the value is intentionally needed.
- Non-secret API-key metadata remains visible, including defaults and configuration identifiers.

## v4.1.0 (2026-08-30)

### New

- Expanded `airs aigateway` to the SDK 0.20.0 resource surface: API keys, audit logs, configs,
  deployments, guardrails, provider integrations, MCP integrations, organisations, plugins,
  providers, complete telemetry reads, and canonical plural `workspaces` commands.
- Added SDK-schema-validated named mutation flags plus repeatable `--set` / `--set-string` dotted
  values for nested configuration. JSON/YAML `--file` input remains an optional advanced base;
  named and dotted flags override it.
- Added confirmation-gated relationship replacement and destructive operations, protected `0600`
  output for one-time credentials, operation-scoped auth/provider redaction, and a live E2E suite
  with read inventory plus an exactly cleaned-up disposable config lifecycle.

### Changed

- Workspace soft removal is canonically `airs aigateway workspaces archive`; the legacy singular
  `workspace delete` spelling remains as a warning-emitting compatibility path and has no `rm`
  alias.
- Pinned `@cdot65/prisma-airs-sdk` to `0.20.0`, using its exported write schemas, known-value
  catalogs, prototype-safe dotted builders, and secret-field metadata.

### Fixed

- Preflight one-time-secret destinations before API mutations, normalize invalid structured input
  to usage errors, make relationship `set` operations replace by default, and apply SDK operation
  metadata to provider and deployment response redaction.

## v4.0.1 (2026-08-29)

### Fixed

- Read commands with legacy Commander defaults now honor the documented output precedence instead
  of allowing an internal `pretty` default to mask global, config-file, or environment selection.
- Empty JSON and YAML lists consistently emit bare arrays (`[]`), including scan-log queries.
- CLI reference tables now describe output as **Resolved** rather than claiming the unconditional
  default is `pretty`, and captured pretty examples state the configuration assumption explicitly.

## v4.0.0 (2026-08-29)

### Changed (breaking)

- **One read-output contract everywhere.** Read commands accept `pretty`, `table`, `markdown`, `csv`, `json`, and `yaml`. JSON/YAML list output is always a bare array of complete normalized records; table/Markdown/CSV use a stable projection. Detail commands emit a complete object in JSON/YAML and a two-column Key/Value view in tabular formats. Status and paging hints go to stderr, leaving stdout pipe-safe.
- **Output selection has one precedence order:** command `--output`, global `--output`, `defaultOutput` in config (or `PANW_CLI_OUTPUT`), then `pretty`.
- **Pagination is uniform.** Paginated lists expose `--limit`, `--offset`, and `--all`; `--all` walks the underlying API pages and is capped at 10,000 records unless `--max` changes the cap (`--max 0` removes it). Profiles and topics return only their latest revision by default; use `--all-versions` or `--revision` when historical data is required.
- **Backup serialization uses `--file-format json|yaml`.** The old `--output` and `--format` spellings remain hidden deprecated aliases for this release.

### New

- Markdown output for read commands, RFC 4180-safe CSV quoting, and camelCase normalized DLP JSON/YAML records.
- AI Gateway cost telemetry retains explicit cent values and adds `totalUsd`, `avgUsd`, and per-record `costUsd` values.
- `airs config get|list` and `airs doctor` participate in the same structured output system.

### Dependencies

- Upgraded `@cdot65/prisma-airs-sdk` to `^0.18.0`, which owns cross-page traversal, maximum-result enforcement, repeated-cursor protection, and latest-revision selection.

## v3.3.0 (2026-08-05)

### New

- **AI Gateway workspace management** — new `airs aigateway workspace` commands cover scoped and tenant-wide listing, detail reads, creation, partial updates, and confirmation-gated archival. Data-plane reads show active workspaces in the caller's SCM role scope; `--plane admin` and `--all` expose tenant-wide active/archived state when the caller has the tenant-root grant.
- **AI Gateway cost telemetry** — `airs aigateway telemetry cost --workspace <slug> [--days 7]` reports total, average, and per-day spend. Pretty output converts AIRS cents to dollars; JSON/YAML keep explicit `*Cents` fields.
- **Red Team custom target adapters** — `airs redteam adapter {list,get,create,update,delete,validate}` manages user-supplied scripts for network-broker targets. Updates preserve omitted variables and stored secrets, validation checks for an ONLINE broker channel, and failed scripts surface `stderr`/`traceback` with exit code 1.

### Fixed

- **Bare STATIC scans now select the full attack catalog** — omitting `--categories` defaults to every available subcategory except `MULTI_TURN`, prints a `--quiet`-aware notice, and avoids the AIRS API's blank 422 response. Library callers must still pass categories explicitly and receive a descriptive local error otherwise.
- **Target scaffolds match current AIRS schemas** — native, REST-family, WebSocket, and custom target adapter templates now use their correct connection shapes, required top-level fields, and adapter variable array.

### Documentation

- Added complete Docusaurus CLI guides for AI Gateway workspaces, cost telemetry, and Red Team adapters; updated scan/target guides, `README.md`, `AGENTS.md`, and `CLAUDE.md` for the new command surfaces and platform rules.

### Dependencies

- Upgraded `@cdot65/prisma-airs-sdk` to `^0.17.0` for AI Gateway and Red Team adapter support.

## v3.2.0 (2026-07-17)

### New

- **Reliable, configurable bulk scanning** — `airs runtime bulk-scan` now accepts `--batch-size <n>` (default `25`, validated as a positive safe integer). Logical batches run sequentially, while each AIRS SDK 0.13.2 call is capped at 20 prompts.
- **Item-level resumable state** — every prompt retains its input index, AIRS `req_id`, status, accepted receipt, and result. State is written before submission and checkpointed throughout the job. Because state contains prompt text, the default directory and files are restricted to modes `0700` and `0600` respectively.

### Fixed

- **One input prompt now produces exactly one correctly correlated output row.** Results are matched by `(scan_id, req_id)` and sorted back into input order, fixing lost, overwritten, or misattributed rows when one scan ID represents several prompts or AIRS returns rows out of order.
- **All runtime outcomes are preserved.** CSV output now includes `topic_violation`, `injection`, `toxic_content`, `dlp`, `url_cats`, `malicious_code`, `source_code`, and `agent`; actions are exactly `allow`, `block`, or `failed`. Failed and timed-out prompts produce failed rows, successful partial results remain available, and the process exits 1 when any prompt failed.
- **Resume is idempotent for known work.** Accepted receipts are polled, only definitely pending items are submitted, and the entire CSV projection is atomically replaced after each completed batch. Repeated resumes no longer append duplicate rows.
- **Overlapping jobs are rejected.** Each state file has an owner lock while bulk/resume work is active, with dead-local-process lock recovery.
- **Ambiguous POST outcomes fail closed.** SDK retries are disabled for async submissions; only confirmed HTTP 429 responses are retried, with `Retry-After` support. Definitive 4xx rejections remain pending, while network and 5xx outcomes are recorded as ambiguous and are never automatically resubmitted. Resume recovers known accepted results before reporting an ambiguity. Exact-once submission cannot be guaranteed after an ambiguous acceptance.
- **Polling is bounded.** The CLI stops after 120 consecutive polls without a newly resolved prompt instead of waiting forever.

### Dependencies

- **`@cdot65/prisma-airs-sdk` 0.13.2 or later is required** for 20-item async submissions, per-call retry control, and structured HTTP/network failure metadata.

## v3.1.0 (2026-07-09)

### New

- **Red Team Network Broker** — manage the data-plane relays that connect red team clients to targets behind a private network. `airs redteam network-broker channels {list,get,create,update}` plus `airs redteam network-broker stats` (server domain, container image/registry, helm chart, client version, online/total channel counts). Channels live on a distinct endpoint, overridable via `PANW_RED_TEAM_NETWORK_BROKER_ENDPOINT` (config key `redTeamNetworkBrokerEndpoint`); OAuth credentials are shared with the other Red Team commands.
- **`airs redteam languages`** — list the tenant's supported languages and job types for multilingual scans. `--management` queries the management plane instead of the data plane.
- **`airs redteam targets error-logs <targetId>`** — list target-profile error logs (timeouts, auth failures, malformed responses captured while a target was exercised).
- **`airs model-security models {list,get,versions,version,files}`** — read-only browsing of the scanned model catalog: models, their versions, and the files within each version, with latest eval outcome, detected formats, source type, and per-file results.

### Changed

- **SDK upgraded to `@cdot65/prisma-airs-sdk` 0.13.0.** Drop-in upgrade (no breaking changes). Also picks up upstream fixes: `customerApps.list()` now percent-encodes the TSG ID, and network-broker `ChannelStats` field names match the live API.

## v3.0.1 (2026-07-07)

- Fixed the Docker image build (v3.0.0 image never published — the tsup config was missing from the build stage). npm package was unaffected.

## v3.0.0 (2026-07-07)

### New

- **`airs doctor`** — credential and connectivity preflight. Checks Node.js version, config file presence/validity, which scanner and management credentials are set (and from which source), scanner API reachability, and management OAuth. Network checks are time-boxed at 5s; prints a pass/warn/fail report with fix hints. Supports `--output json|yaml`. Exits 0 when healthy (warnings OK), 1 on any failure.
- **`airs config {list,get,set,unset,path}`** — manage `~/.prisma-airs/config.json` from the CLI: effective-config listing with per-key source (env/file/default), schema-validated `set`, round-trip-safe `unset` that preserves unknown file keys, secret masking with `--reveal` opt-out, and a `PRISMA_AIRS_CONFIG_PATH` env override for the config file location.
- **`airs completion <bash|zsh|fish>`** — shell completion scripts with install snippets.
- **Global `--quiet` flag** — suppresses status and decorative output while keeping data, results, and errors.
- **Confirmation prompts on destructive operations** — profiles/topics/targets delete, topics revert, and profiles cleanup now ask Y/N before proceeding. `--force` bypasses; non-interactive runs without `--force` exit 2.
- **`ls`/`rm` aliases** on every `list`/`delete` subcommand, and usage examples in `--help` for the most-used commands.
- **Endpoint and auth overrides (full SDK parity)** — new config keys `airsApiToken` (bearer-token alternative to the scan API key), `airsApiEndpoint`, `airsNumRetries`, `redTeamDataEndpoint`, `redTeamMgmtEndpoint`, `redTeamTokenEndpoint`, `modelSecDataEndpoint`, `modelSecMgmtEndpoint`, `modelSecTokenEndpoint`. Scan commands accept `PANW_AI_SEC_API_TOKEN` in place of `PANW_AI_SEC_API_KEY`.

### Changed

- **Flag standardization** — `--output` always means format; file destinations are `--output-file`; input files are `--file`; pagination is `--limit`/`--offset`; destructive bypass is `--force`. Old spellings (`--format`, `--input`, `--page`/`--size`, `--confirm`) keep working throughout v3 as hidden aliases with a stderr deprecation notice and will be removed in v4 — see the [Flag Migration guide](flag-migration.md). Also new: `--output pretty|json|yaml` on `redteam prompts list|get`, `redteam instances get`, `redteam registry-credentials`, and client-side `--limit`/`--offset` on redteam list commands.
- **Pipe-safe machine-readable output** — `--output json|yaml|csv` emits only the payload on stdout; progress, banners, and rate-limit warnings moved to stderr, so `--output json | jq` always parses. Exit codes standardized across every command group: 0 success, 1 runtime/API failure, 2 usage error. API errors show the HTTP status and a `--debug` hint.
- **CLI output design system** — all renderers (backup, eval, redteam, runtime, dlp, model-security) migrated to shared `ui` primitives: uniform bold headers, semantic glyphs (✓ ✗ ⚠ ○ ● •), aligned key/value blocks, canonical box-drawing tables, and standardized `No <resource> found` empty-list phrasing.
- **~6x faster startup** (≈0.4s → ≈0.06s) — the DLP test-file generator dependencies (sharp, pdf-lib, docx, piexifjs) now load lazily, only when `airs runtime dlp generate` runs, and moved to optionalDependencies. Installs with `--no-optional` skip ~50MB of native binaries; `dlp generate` prints an install hint if they are absent.
- **Bundled build (tsup)** — dist/ went from ~200 files (2.5MB) to 5 files (355KB unpacked), with the DLP generator split into a lazy chunk. No API changes — library entry, types, and CLI bin paths are unchanged.
- **Hardened `--debug` logging** — sensitive request/response body fields, query parameters, and headers are fully masked before hitting the debug JSONL file (previously only two headers were partially masked). Debug logs rotate automatically, keeping the 10 newest. Unhandled promise rejections print a friendly error instead of a raw crash.

### Changed (breaking)

- **`airs runtime dlp-gen` moved to `airs runtime dlp generate`.** The DLP test-file generator now lives under the `dlp` namespace alongside `dictionaries`, `filtering-profiles`, `patterns`, and `profiles`. Flags and behavior are unchanged — `--types`, `--count`, `--out`, `--techniques`, `--seed`, `--output` work identically. Update any scripts or aliases that called `airs runtime dlp-gen`.

### Removed (breaking)

- **External LLM functionality removed.** Custom topic guardrail generation is now fully agent-driven (see `AGENTS.md` / `CLAUDE.md`), so the LLM provider layer is no longer needed.
  - Removed the `airs runtime profiles audit` command (it used an LLM to generate test prompts).
  - Removed the LLM provider configuration: `--provider` / `--model` flags, the `llmProvider` / `llmModel` config fields, and the `ANTHROPIC_API_KEY`, `GOOGLE_API_KEY`, `GOOGLE_CLOUD_*`, `AWS_*`, `LLM_PROVIDER`, `LLM_MODEL` environment variables. AIRS scanner + management credentials are unchanged.
  - Removed the **LLM Providers** documentation section.
  - Library: removed the `audit` exports (`runAudit`, `computeTopicAuditResults`, `computeCompositeMetrics`, `detectConflicts`, `buildAuditReportJson`, `buildAuditReportHtml`) and the orphaned run-report exports (`buildReportJson`, `buildReportHtml`). `ProfileTopic` is retained.
- **`airs runtime dlp-profiles list` removed.** Use `airs runtime dlp profiles list` (DLP namespace) instead — it is now the canonical listing and returns populated profile IDs plus `type`, `profile_type`, `status`, and `version` fields, paginated as `{items, page:{number,size,total,returned}}`.

#### Migration note

The two endpoints overlap heavily but are **not identical** on the same tenant. Before switching scripts that consumed the legacy command, verify the profiles you rely on are present in the new output:

- The legacy Management endpoint may have surfaced profiles the new DLP namespace endpoint does not (observed on at least one tenant: `PII Basic Block All Data` was legacy-only).
- The new DLP namespace endpoint may surface profiles the legacy endpoint did not (observed: `Malware` was new-only).
- The new endpoint is paginated — pass `--page` / `--size` to walk past the first page.
- Field shape changes: legacy returned `[{id,name}]` with empty IDs; new returns `{items:[{id,name,type,profile_type,status,version}], page:{...}}`.

See [#226](https://github.com/cdot65/prisma-airs-cli/issues/226) for the divergence analysis.

---

## v2.10.0

### Changed

- **DLP write commands now take structured flags** — `patterns|profiles|filtering-profiles create/replace` accept `--name`, `--regex`, `--weighted-regex`, `--pattern-id`, `--file-based`, `--direction`, `--tag k=v`, etc. instead of forcing `--body-file pattern.json`. `--body`/`--body-file` retained as escape hatches for complex rule trees.
- **DLP output curated across all formats** — `--output json|yaml` now returns `{items, page:{number,size,total,returned}}` for lists and `{action,id,name,type,status,version}` for acks, dropping the raw SDK envelope leak (`tenant_id`, `is_parent_managed`, `pageable.*`).

### Fixed

- **`dlp dictionaries create` now honors `--output`** — was hardcoded to `pretty`, ignoring the flag. Now matches the rest of the DLP command surface.

---

## v2.9.0

### New

- **DLP command group** — `airs runtime dlp` adds full CRUD across four DLP subclients:
  - `filtering-profiles` (list/get/replace)
  - `patterns` (list/create/get/replace/patch/soft-delete)
  - `profiles` (list/create/get/replace/patch — no delete; archive via patching `profile_status`)
  - `dictionaries` (full CRUD with multipart upload; handles both 200+body and 204+empty replace responses)
- Optional `PANW_DLP_ENDPOINT` env var (defaults to SDK built-in).

### Fixed

- **`--debug` now captures DLP traffic** — fetch interceptor's host allowlist was missing `api.dlp.paloaltonetworks.com`, so `runtime dlp` commands were silently bypassing the JSONL log.

### Dependencies

- `@cdot65/prisma-airs-sdk` bumped to `^0.9.2` (DLP nested helper nullable sweep — unblocks `runtime dlp patterns list` and `runtime dlp profiles list` against live tenants).

---

## v2.4.0

### New

- **Profile cleanup** -- `airs runtime profiles cleanup` deletes old profile revisions, keeping only the latest revision per profile name. AIRS creates a new revision (with a new UUID) on every profile update; this command prunes the accumulated duplicates. Supports `--force` to skip confirmation, `--updated-by <email>` (defaults to `git config user.email`), and `--output json` for structured output.

---

## v2.3.0

### New

- **Target init from templates** -- `airs redteam targets init <provider>` scaffolds a target config JSON from provider templates (OPENAI, HUGGING_FACE, DATABRICKS, BEDROCK, REST, STREAMING). Supports `--output <file>` for custom paths.

---

## v2.2.0

### New

- **EULA management** -- `airs redteam eula {status,content,accept}` for checking, viewing, and accepting the Red Team end-user license agreement
- **Instance management** -- `airs redteam instances {create,get,update,delete}` for managing Red Team compute instances
- **Device management** -- `airs redteam devices {create,update,delete}` for managing devices attached to instances
- **Registry credentials** -- `airs redteam registry-credentials` for fetching container registry tokens
- **Target auth validation** -- `airs redteam targets validate-auth` to test auth credentials without modifying targets
- **Target metadata** -- `airs redteam targets metadata` to retrieve field metadata and validation rules
- **Target templates** -- `airs redteam targets templates` to get provider-specific configuration templates

### Fixed

- **Bulk scan polling hang** -- async query API returns lowercase `"complete"`/`"failed"` but polling checked for uppercase `"COMPLETED"`/`"FAILED"`, causing infinite loop. Status comparison is now case-insensitive.

### Dependencies

- `@cdot65/prisma-airs-sdk` bumped to `^0.7.0` (Red Team EULA, instances, target auth/metadata/templates, WebSocket support)

---

## v2.1.0

### New

- **Intent-aware eval CSV format** — eval CSV now requires three columns: `prompt`, `expected`, `intent` (block/allow). The `expected` column is intuitive (belongs to topic category: true/false) and `intent` controls the trigger mapping.
- **`airs runtime topics sample` command** — prints a template CSV showing the three-column format with both block and allow intent examples. Supports `--output <path>` to write to file.
- **Agent instruction ecosystem** — rewritten `program.md` with battle-tested optimization protocol. New agent entrypoints: `GEMINI.md`, `.github/copilot-instructions.md`. Any AI coding agent can now pick up the guardrail optimization loop.
- **JSON eval output includes intent** — `--format json` output now includes an `intent` field at the top level.

### Changed

- `topics create` flags: `--name`, `--description`, `--examples` replace the old `--topic` flag
- `topics apply` flags: `--name` replaces `--topic`, `--intent` added
- `topics eval` flags: `--prompts` replaces `--input`, `--format` replaces `--output`
- `topics revert` flags: `--name` replaces `--topic`
- Updated `AGENTS.md` with correct flag names, sample command, and three-column CSV docs
- Updated all mkdocs pages to reflect new CLI flags and CSV format

### Breaking

- Eval CSV files must now include an `intent` column. Existing two-column CSVs will error with "Missing required column: intent".

## v2.0.0

### Changed

- Major refactor: removed embedded LLM-driven generation loop. CLI now provides atomic commands (`create`, `apply`, `eval`, `revert`) for external agent orchestration.
- Removed `topics generate`, `topics resume`, `topics report`, `topics runs` commands.
- Removed memory/persistence subsystem.

## v1.4.2

### Fixed

- Profile create now includes AIRS UI-required defaults: `app-protection`, `data-protection`, `latency`, `mask-data-in-storage`
- `--toxic-content alert` expands to `"high:alert, moderate:alert"` (AIRS UI expects `severity:action` format)
- Fixes "is not iterable" crash in AIRS UI when viewing CLI-created profiles

## v1.4.1

### Fixed

- `profiles delete` by UUID now shows profile name in success message
- `profiles create` handles AIRS 409 race — detects successful creation despite SDK error
- `profiles create` defaults latency config to `block` / `5s` when not explicitly set

## v1.4.0

### New

- **`--rate <n>` flag for generate/resume** — caps AIRS scan API calls to N per second during guardrail generation and resumed runs. Uses a sliding-window token bucket. Default: unlimited. Prevents hitting API rate limits during intensive scan loops.
- **`--debug` global flag** — logs all AIRS and Strata Cloud Manager API requests and responses to a JSONL file (`~/.prisma-airs/debug-api-<timestamp>.jsonl`) for offline inspection and sharing with Palo Alto Networks support. Works with any subcommand across all three command groups. Auth tokens are redacted.

### Fixed

- `profiles delete` and `profiles update` now accept name or UUID (same auto-detect as `profiles get`)
- `profiles delete` prints proper success message instead of `undefined`
- `profiles create` gives actionable error on 409 conflict: suggests `profiles update`

## v1.3.0

### New

- **Docs restructured by AIRS module** — navigation reorganized into Runtime Security, AI Red Teaming, and Model Security top-level sections instead of flat Capabilities/Guides layout
- **Profile create/update CLI flags** — `profiles create` and `profiles update` now use 20+ CLI flags (`--prompt-injection`, `--toxic-content`, `--malicious-code`, etc.) instead of `--config` JSON files
- **Read-modify-write profile updates** — `profiles update` fetches current profile, merges only specified flags, then PUTs full payload (no config overwrites)

### Changed

- Docs site navigation: features/ and examples/ directories merged into runtime/, redteam/, model-security/ module sections
- Architecture and LLM Providers moved under Reference tab

## v1.2.0

### New

- **Profile builder** — converts CLI flags to `CreateSecurityProfileRequest`, supports all protection flags
- **`mergeProfilePolicy()`** — deep-merges CLI flag overrides into existing profile policy for PUT-only API
- **`profiles create`** — create security profiles with CLI flags for all protection categories
- **`profiles update`** — update profiles with read-modify-write pattern; only specify what changes
- **`profiles delete --force --updated-by`** — force deletion of profiles with dependencies

## v1.1.0

### New

- **`profiles get` command** — retrieve full security profile configuration by name or UUID
    - Auto-detects UUID vs profile name
    - Supports `--output pretty|json|yaml`
    - Shows complete policy JSON (topic guardrails, DLP, app protection, etc.)
- Bump `@cdot65/prisma-airs-sdk` to v0.6.10

## v1.0.9

### Fixed

- Make `changeType` optional in learning extraction schema — LLM omits it for neutral-outcome learnings, causing `OUTPUT_PARSING_FAILURE` during memory extraction. Defaults to `'initial'` when omitted.

## v1.0.8

### Fixed

- Remove unused `OUTPUT_FORMATS` import in redteam.ts
- Add missing `intent` parameter to improveTopic test
- Update langchain ecosystem to resolve `standard_schema` export crash

### Dependencies

- `@cdot65/prisma-airs-sdk` 0.6.3 → 0.6.7
- `@langchain/aws` 1.3.0 → 1.3.3
- `@langchain/core` 1.1.29 → 1.1.34
- `@langchain/anthropic` 1.3.21 → 1.3.25
- `@langchain/google-genai` 2.1.22 → 2.1.26
- `@langchain/google-vertexai` 2.1.22 → 2.1.26

### Security

- Resolved transitive `fast-xml-parser` CVE via `@langchain/aws` update

## v1.0.7

### Fixed

- Display full API key value on create/regenerate
- Show last 8 characters of API key in list and detail views

## v1.0.6

### New

- **`--output` flag on all list commands** — unified structured output across all 3 command groups
    - Formats: `pretty` (default), `table`, `csv`, `json`, `yaml`
    - Supported on: `runtime profiles list`, `runtime topics list`, `runtime api-keys list`, `runtime customer-apps list`, `runtime deployment-profiles list`, `runtime dlp-profiles list`, `runtime scan-logs query`, `redteam list`, `redteam targets list`, `redteam prompt-sets list`, `redteam properties list`, `model-security groups list`, `model-security rules list`, `model-security scans list`

## v1.0.5

### New

- **`airs model-security install`** — one-command setup of the `model-security-client` Python package from AIRS private PyPI
    - Auto-detects `uv` (uses `uv init` + `uv add`) or falls back to `python3 -m venv` + `pip install`
    - `--extras` for source type selection: `all`, `aws`, `gcp`, `azure`, `artifactory`, `gitlab`
    - `--dir` to specify project directory
    - `--dry-run` to preview commands

### Fixed

- CLI help menus now display subcommands in alphabetical order across all command groups

## v1.0.0

First release of Prisma AIRS CLI (renamed from `daystrom`). See [MIGRATION.md](https://github.com/cdot65/prisma-airs-cli/blob/main/MIGRATION.md) for upgrade steps.

### Highlights

- **5 capability domains**: Runtime Security scanning, Guardrail Generation with iterative refinement, AI Red Teaming, Model Security scanning, Profile Audits
- **Runtime configuration management**: Full CRUD for security profiles, custom topics, API keys, customer apps, deployment/DLP profiles, scan logs
- **Guardrail generation loop**: LLM-driven topic refinement with two-phase generation, test composition, weighted category generation, 3-tier recovery, plateau detection
- **AI Red Teaming**: Static/dynamic/custom scans, target CRUD with connection validation, prompt set management, property management
- **Model Security**: Security groups CRUD, rule browsing, rule instance configuration, scan operations with evaluations/violations/files, label management
- **Profile Audits**: Multi-topic evaluation with per-topic metrics, cross-topic conflict detection, JSON/HTML report export
- **6 LLM providers**: Claude (API, Vertex, Bedrock) and Gemini (API, Vertex, Bedrock)
- **Cross-run learning memory** with keyword categorization and budget-aware prompt injection
- **Structured evaluation reports**: JSON, HTML, and terminal formats with run comparison (`--diff`)
- **Resumable runs** with full state persistence
- **537 tests** across 29 spec files
- **Docker support** with multi-arch images (amd64 + arm64)

### CLI Structure

```
airs runtime scan            # Sync scan
airs runtime bulk-scan       # Async bulk scan
airs runtime resume-poll     # Resume polling
airs runtime profiles ...    # Security profile CRUD
airs runtime topics ...      # Custom topic CRUD + guardrail generation
airs runtime api-keys ...    # API key management
airs runtime customer-apps   # Customer app CRUD
airs runtime deployment-profiles  # Deployment profile listing
airs runtime dlp-profiles    # DLP profile listing
airs runtime scan-logs       # Scan log querying
airs redteam scan            # Launch red team scan
airs redteam targets ...     # Target CRUD
airs redteam prompt-sets ... # Prompt set CRUD
airs redteam prompts ...     # Individual prompt CRUD
airs redteam properties ...  # Property management
airs model-security groups    # Security group CRUD
airs model-security install   # Install model-security-client Python package
airs model-security labels    # Label management
airs model-security rules     # Rule browsing
airs model-security scans     # Scan operations
```

### Breaking Changes (from daystrom)

- CLI binary renamed: `daystrom` → `airs`
- Package renamed: `@cdot65/daystrom` → `@cdot65/prisma-airs-cli`
- Data directory: `~/.daystrom/` → `~/.prisma-airs/`
- Guardrail commands moved under `airs runtime topics`
- Audit command moved under `airs runtime profiles audit`
- Deprecated top-level aliases removed — use `airs runtime topics` and `airs runtime profiles` subcommands
