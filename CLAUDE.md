# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Summary

Prisma AIRS CLI (`airs`) is a CLI and library providing full operational coverage over **Palo Alto Prisma AIRS** AI security capabilities: runtime prompt scanning and configuration management, atomic topic commands (create, apply, eval, revert) for agent-driven optimization following the autoresearch pattern, adversarial red team scanning, ML model supply chain security, and backup/restore of AIRS configuration to local files.

## Commands

```bash
# Dev
pnpm install               # Install deps
pnpm run build             # tsup bundle to dist/ (esm, split chunks; build:tsc for raw tsc)
pnpm run dev               # Run CLI via tsx (any subcommand)

# Test
pnpm test                  # All tests (vitest run)
pnpm test:watch            # Watch mode
pnpm test -- tests/unit/core/metrics.spec.ts   # Single file
pnpm test -- -t "pattern"  # Tests matching name pattern
pnpm test:coverage         # Coverage (excludes src/cli/**, src/index.ts, **/types.ts)
pnpm test:e2e              # E2E tests (requires real creds, opt-in)

# Docker (Dockerfile lives at docker/Dockerfile)
pnpm run docker:build      # Build local image (-f docker/Dockerfile)
pnpm run docker:run        # Run (mounts ~/.prisma-airs)

# Lint & Format (Biome config lives at config/biome.json)
pnpm run lint              # Biome check  --config-path config
pnpm run lint:fix          # Biome check  --write --config-path config
pnpm run format            # Biome format --write --config-path config
pnpm run format:check      # Biome format --config-path config (check only)

# Type-check
pnpm tsc --noEmit

# Docs (Docusaurus site in docs-site/ + TypeDoc API)
pnpm run docs:api          # Generate TypeDoc markdown API reference
pnpm run docs:build        # docs:api + build docs-site
pnpm run docs:serve        # docs:api + serve docs-site locally
```

## Releases

**Always cut versions via `pnpm changeset version`. Never hand-edit `package.json`.**

This repo keeps **no root change-log file** — changesets are configured with `"changelog": false` (`.changeset/config.json`), so `changeset version` only bumps `package.json` and consumes the queued `.changeset/*.md` files; it does not generate a change log. User-facing release notes are hand-maintained in `docs-site/docs/about/release-notes.md` plus the GitHub Release body.

Workflow when shipping a release:

1. `pnpm changeset version` — consumes queued `.changeset/*.md` → bumps `package.json` based on the highest bump type across queued entries (`major` > `minor` > `patch`). No changelog is generated.
2. Commit `package.json` + the deleted changeset files together: `chore(release): X.Y.Z — <short title>`.
3. Update `docs-site/docs/about/release-notes.md` with the user-facing notes for this version.
4. Tag `vX.Y.Z` and push commit + tag.
5. `gh release create vX.Y.Z --title "vX.Y.Z — <title>" --notes ...` — this fires `.github/workflows/publish.yml`, which runs lint/typecheck/test/build and `npm publish` via OIDC.

For hotfixes that should bypass all queued changesets and ship only one fix: add only the hotfix's changeset, branch off the release tag (not `main`), run `pnpm changeset version`, release, then rebase/merge back. Manual `package.json` edits are only acceptable as a last resort (e.g. the queued backlog is corrupted) and must be paired with a follow-up cleanup PR.

## Agent-Driven Optimization

The `topics create/apply/eval/revert` commands are designed for autonomous agent loops (autoresearch pattern). An agent can: create a topic, apply it to a profile, eval against a static prompt set, keep or revert based on FP/FN metrics, and repeat. See `AGENTS.md` for the agent loop protocol.

## Code Style (Biome)

Single quotes, semicolons, 2-space indent, 100-char line width. Imports auto-organized.

## Coverage Thresholds

Lines: 90%, Functions: 95%, Branches: 80%, Statements: 90%. Coverage excludes `src/cli/**`, `src/index.ts`, `**/types.ts`.

## Directory Structure

Top-level layout: `src/` (library + CLI), `tests/`, `config/biome.json` (Biome config), `docker/` (`Dockerfile`, `docker-push-arm64.sh`), `docs-site/` (Docusaurus documentation site), `typedoc.json` (API-docs generator). There is no top-level build-scripts directory and no root change-log file.

```
src/
├── cli/                   # CLI entry, command groups, renderer
│   ├── index.ts           # CLI entry — installs process guards, builds and runs the program
│   ├── program.ts         # buildProgram() — registers all command groups, global --debug/--quiet flags, ls/rm aliases on every list/delete
│   ├── process-guards.ts  # Unhandled-rejection/uncaught-exception handlers → friendly error, exit 1
│   ├── deprecated-flags.ts # Hidden v2 flag aliases (--format/--input/--page…) → canonical flag + stderr deprecation notice
│   ├── pagination.ts      # Shared --limit/--offset options + page conversion for page-based APIs
│   ├── confirm.ts         # Interactive confirmation for destructive commands (--force bypasses)
│   ├── examples.ts        # Usage-example help text formatter (.addHelpText)
│   ├── debug-logger.ts    # Global fetch interceptor — logs AIRS/SCM API traffic to JSONL (deep redaction, keeps 10 newest files)
│   ├── builders/
│   │   └── profile-builder.ts # CLI flags → CreateSecurityProfileRequest builder + merge utility
│   ├── commands/
│   │   ├── topics-create.ts  # Create or update a custom topic (validates constraints, upserts by name)
│   │   ├── topics-apply.ts   # Assign topic to profile (additive, preserves existing topics)
│   │   ├── topics-eval.ts    # Scan static prompt set, compute metrics, return FP/FN lists
│   │   ├── topics-revert.ts  # Remove topic from profile and delete it
│   │   ├── topics-sample.ts  # Print sample eval CSV (prompt,expected,intent)
│   │   ├── backup.ts      # Backup core logic (backupTargets, createRedTeamService, toBackupData)
│   │   ├── restore.ts     # Restore core logic (restoreTargets, prepareTargetPayload)
│   │   ├── profiles-cleanup.ts # Delete old profile revisions, keep only latest per name
│   │   ├── dlp/           # DLP CLI commands (4 subgroups + aggregator + shared patch/parseBody utils)
│   │   ├── config.ts      # airs config {list,get,set,unset,path} — manage ~/.prisma-airs/config.json
│   │   ├── doctor.ts      # airs doctor — environment/credential/connectivity diagnostics
│   │   ├── completion.ts  # airs completion <shell> — shell completion scripts
│   │   ├── runtime.ts     # Runtime scanning + config management + topics (profiles)
│   │   ├── redteam.ts     # Red team operations (scan, targets CRUD + backup/restore, prompt-sets CRUD, prompts CRUD, properties)
│   │   └── modelsecurity.ts # Model security operations (groups, rules, rule-instances, scans, labels, pypi-auth)
│   ├── bulk-scan-state.ts # Validated item-centric v2 bulk state; atomic 0600 checkpoints for safe resume
│   ├── parse-input.ts     # Input file parsing — CSV (prompt column) or plain text (line-per-prompt)
│   └── renderer/          # Terminal output, split by command group — renderers compose ui.ts primitives (no direct chalk)
│       ├── index.ts       # Barrel re-exports
│       ├── ui.ts          # Design-system primitives (status/success/warn/error/section/table/keyValue…) + quiet mode
│       ├── DESIGN.md      # Renderer design-system spec (stdout=data, stderr=status; ui primitive contract)
│       ├── backup.ts      # Backup/restore summary rendering
│       ├── common.ts      # renderError
│       ├── eval.ts        # Eval metrics, FP/FN list rendering
│       ├── redteam.ts     # Red team scan/target/prompt-set rendering
│       ├── runtime.ts     # Runtime scan + config management rendering
│       ├── dlp.ts         # DLP filtering-profiles/patterns/profiles/dictionaries rendering
│       └── modelsecurity.ts # Model security groups/rules/scans rendering
├── config/
│   ├── schema.ts          # Zod ConfigSchema — all config fields w/ defaults
│   └── loader.ts          # Config cascade: CLI > env > file > Zod defaults
├── core/
│   ├── prompt-loader.ts   # Load static prompt set from CSV/text for eval command
│   ├── types.ts           # CustomTopic, UserInput, TestCase, TestResult, EfficacyMetrics, AnalysisReport, IterationResult, RunState
│   ├── metrics.ts         # computeMetrics() — TP/TN/FP/FN → TPR/TNR/accuracy/coverage/F1
│   └── constraints.ts     # AIRS topic limits: 100 name, 250 desc, 250/example, 5 max, 1000 combined
├── airs/
│   ├── scanner.ts         # AirsScanService + DebugScanService + RateLimitedScanService — syncScan + scanBatch
│   ├── runtime.ts         # SdkRuntimeService — sync scan, correlated async batches, bounded polling, CSV export
│   ├── management.ts      # SdkManagementService — topic CRUD, profile CRUD, API keys, customer apps, deployment profile listing, scan logs
│   ├── promptsets.ts      # SdkPromptSetService — custom prompt set CRUD via RedTeamClient
│   ├── dlp/               # DLP namespace: filtering-profiles, patterns, profiles, dictionaries SDK service wrappers
│   ├── redteam.ts         # SdkRedTeamService — red team scan CRUD, polling, reports
│   ├── modelsecurity.ts   # SdkModelSecurityService — security groups, rules, scans, labels
│   └── types.ts           # ScanResult, ProfileTopic, ScanService, ManagementService, PromptSetService, RedTeamService, ModelSecurityService
├── backup/
│   ├── types.ts           # BackupEnvelope<T>, BackupFormat, ResourceType, result types
│   ├── io.ts              # writeBackupFile, readBackupFile, readBackupDir, sanitizeFilename
│   └── index.ts           # Barrel exports
├── dlp/                   # DLP test-file generator (synthetic sensitive data; powers `runtime dlp generate`)
│   ├── generate/          # Clean carrier-file generators per format (pdf, png, jpeg, svg, docx, raster)
│   ├── embed/             # "Dirty" embedding techniques per format (metadata, hidden-text, stego, exif, zip, overlay…)
│   ├── lorem.ts           # Filler text generation
│   ├── manifest.ts        # Manifest (dirty file → technique + embedded values, for scoring)
│   ├── payload.ts         # Synthetic sensitive-value payloads
│   ├── rng.ts             # Seeded RNG for reproducible output
│   └── types.ts           # DLP generation types
└── index.ts               # Library exports

tests/
├── integration/           # Public CLI bulk-scan and installed SDK runtime-contract tests
├── unit/                  # spec files
│   ├── airs/              # scanner.spec.ts, management.spec.ts, modelsecurity.spec.ts, promptsets.spec.ts, redteam.spec.ts, runtime.spec.ts
│   ├── backup/            # io.spec.ts
│   ├── cli/               # parse-input.spec.ts, bulk-scan-state.spec.ts, backup.spec.ts, backup-renderer.spec.ts, restore.spec.ts
│   ├── config/            # schema.spec.ts, loader.spec.ts
│   ├── core/              # metrics.spec.ts, constraints.spec.ts
│   └── dlp/               # DLP service + CLI command specs
└── helpers/               # mocks.ts
```

## Architecture

### CLI Conventions (`src/cli/`)
- **Exit codes**: 0 success, 1 operational error (API/network/runtime), 2 usage error (bad flags/validation)
- **Output discipline**: stdout carries data only; status/decorative output goes to stderr — `--output json | jq` always parses
- **`--quiet` global flag**: suppresses status/decorative output; data and errors still print
- **Flag canon**: `--output` = format (`pretty|table|csv|json|yaml`), `--output-file`/`--output-dir` = destinations, `--file`/`--input-dir` = inputs, `--limit`/`--offset` = pagination, `--force` = skip confirmation. Old v2 spellings (`--format`, `--input`, `--page`/`--size`, `--confirm`) are hidden deprecated aliases, removed in v3 — see `docs-site/docs/about/flag-migration.md`
- **Confirmation prompts**: destructive commands prompt interactively unless `--force` (non-TTY requires `--force`)
- **Aliases**: every `list` command accepts `ls`, every `delete` accepts `rm`
- **Utility commands**: `airs config {list,get,set,unset,path}` (config file management), `airs doctor` (env/credential/connectivity diagnostics), `airs completion <shell>` (shell completions)

### Topic Commands (`src/cli/commands/topics-*.ts`)
- **`create`** (`topics-create.ts`): create or update a custom topic; validates AIRS constraints (name ≤100, desc ≤250, each example ≤250, combined ≤1000, max 5 examples), upserts by name
- **`apply`** (`topics-apply.ts`): assign topic to a security profile; additive — reads current profile topic-list, appends the new topic with correct `revision`, writes back; never clobbers existing topics
- **`eval`** (`topics-eval.ts`): load a static prompt set (CSV or text), scan each prompt against the named profile, compute TP/TN/FP/FN → TPR/TNR/coverage/F1, return FP and FN lists for agent inspection
- **`revert`** (`topics-revert.ts`): remove topic from profile topic-list and delete the topic; safe — checks profile reference before deleting

These four commands compose into an autoresearch-style optimization loop: an agent calls `create → apply → eval`, decides keep or `revert`, then iterates.

### Backup & Restore (`src/backup/`, `src/cli/commands/backup.ts`, `src/cli/commands/restore.ts`)
- `airs redteam targets backup` — export all or single target to local JSON/YAML files
- `airs redteam targets restore` — import targets from backup files, skip or overwrite existing
- Backup envelope: `{ version, resourceType, exportedAt, data }` — server fields (uuid/status/active/version) stripped on restore via `prepareTargetPayload()`
- Backup data uses API field names: `target_background` (not `background`), `target_metadata` (not `metadata`); legacy names auto-normalized on restore
- Shared I/O utilities in `src/backup/io.ts` — extensible to future resource types (profiles, topics, prompt-sets)
- CLI: `airs redteam targets backup [--output-dir <path>] [--output json|yaml] [--name <name>]`
- CLI: `airs redteam targets restore [--input-dir <path>] [--file <path>] [--overwrite] [--validate]`

### AIRS Integration (`src/airs/`)
- **Scanner**: `Scanner.syncScan()` via SDK, detection = `prompt_detected.topic_violation === true` (sole signal, no fallbacks)
- **Detection**: `triggered` (= `topic_violation`) is the sole guardrail detection signal. No category-based or action-based detection.
- **`DebugScanService`**: Wrapper that appends raw scan responses to a JSONL file when `--debug-scans` is passed
- **`RateLimitedScanService`**: Wrapper that caps scan throughput to N calls/second via sliding-window token bucket
- **`--debug` global flag**: Intercepts `globalThis.fetch` to log all AIRS/SCM API requests and responses to `~/.prisma-airs/debug-api-<timestamp>.jsonl`. Deep redaction — sensitive headers, query params, and credential-like body fields masked as `***`; only the 10 newest debug files are kept. Works with any subcommand.
- **Prompt sets**: `SdkPromptSetService` wraps `RedTeamClient.customAttacks` for custom prompt set CRUD
- **Management**: `ManagementClient` via OAuth2 — topic CRUD, security profile CRUD, API key management, customer app management, deployment profile listing, scan log querying
- Profile updates create **new revisions with new UUIDs** — always reference profiles by name, never ID
- Topics must be added to profile's `model-protection` → `topic-guardrails` → `topic-list`
- AIRS rejects empty `topic-list` entries — only include entries with topics
- **CRITICAL: topic-list `revision` field**: AIRS pins topic content to the `revision` number in the profile's topic-list. Omitting it defaults to revision 0 (original creation content). `topics apply` fetches current topic revisions via `listTopics()` and includes them.
- **CRITICAL: always scan by profile NAME**, never by profile ID/UUID. Scanning by name always uses the latest profile version; scanning by ID pins to a versioned snapshot.
- Topics can't be deleted while referenced by any profile revision
- **Platform ceilings**: Topics in high-sensitivity domains (explosives, weapons) trigger built-in AIRS safety that overrides custom definitions. Shorter descriptions generally outperform longer ones with exclusion clauses.

### Runtime Scanning (`src/airs/runtime.ts`)
- `SdkRuntimeService` wraps SDK `Scanner` for sync and async scanning; async reliability depends
  on `@cdot65/prisma-airs-sdk` **0.13.2 or newer** for 20-item async submissions, per-call retry control, and structured failure
  metadata (`failureKind`, `statusCode`, `retryAfterMs`)
- `scanPrompt()` — sync scan via `syncScan()`, normalizes to the backward-compatible
  `RuntimeScanResult` (`allow`/`block`); richer bulk outcomes use `BulkScanResult`
- **Detection scope**: runtime results aggregate 8 detection types via OR (`topic_violation`,
  `injection`, `toxic_content`, `dlp`, `url_cats`, `malicious_code`, `source_code`, `agent`).
  This is intentionally broader than the guardrail loop's `topic_violation`-only signal — runtime
  scanning is a general-purpose firewall check, not topic-specific evaluation.
- `submitBatch()` — accepts exactly 1–20 indexed prompts and makes one `asyncScan()` call. It passes
  `{ numRetries: 0 }` to the SDK, then performs its own bounded retries only for confirmed HTTP 429
  failures, honoring `retryAfterMs`/`Retry-After`.
- `pollBatch()` — polls one receipt until every prompt resolves, mapping by `(scan_id, req_id)` and
  falling back to `queryByReportIds()` if a terminal scan row lacks `req_id`. Polling uses SDK
  retries 0, honors rate-limit metadata, and fails after a bounded number of no-progress polls.
- On the reliable bulk path, the only actions are `allow`, `block`, and `failed`; failed/timeout
  responses become explicit `action: 'failed'` results. They are never normalized to `allow`;
  partial successes remain in state/output and the CLI exits 1 when any prompt failed.
- `formatResultsCsv()` — deterministic CSV projection with all eight detector columns and an
  operational `error` column
- CLI: `airs runtime scan --profile <name> [--response <text>] <prompt>`
- CLI: `airs runtime bulk-scan --profile <name> --file <file> [--output-file <file>] [--session-id <id>] [--batch-size <n>]`
- Input file parsing: `.csv` files extract the `prompt` column by header; `.txt`/extensionless use line-per-prompt
- The command saves item-centric v2 state **before the first POST** and around each submission.
  State includes `batchSize`, prompt text, per-item status/receipt/result, and timestamps. Default
  state directories are `0700`; state files are atomically replaced with mode `0600` because prompt
  text is sensitive.
- Submission outcome policy: a definite HTTP 4xx leaves an item `pending` (429 is first retried by
  the CLI); network and HTTP 5xx failures are `ambiguous`. Ambiguous/submitting items are never
  automatically resubmitted because the POST may have been accepted.
- The CSV is a full, ordered projection of completed state, atomically replaced after each completed
  SDK batch. Resume is idempotent and cannot duplicate rows by appending.
- Bulk and resume invocations hold a per-state lock, reject overlapping processes, and recover a
  stale lock when its local owner process no longer exists.
- CLI: `airs runtime resume-poll <stateFile> [--output-file <file>]` — polls accepted receipts,
  resubmits only definitely unaccepted `pending` items, and fails closed on ambiguous submissions
- CLI config management subcommand groups (all via `ManagementClient` OAuth2):
  - `airs runtime profiles {list,get,create,update,delete,cleanup}` — security profile CRUD + revision cleanup
    - `get` accepts name or UUID, supports `--output pretty|json|yaml`
    - `create` requires `--name`, plus optional protection flags: `--prompt-injection`, `--toxic-content`, `--contextual-grounding`, `--malicious-code`, `--url-action`, `--allow-url-categories`, `--block-url-categories`, `--alert-url-categories`, `--agent-security`, `--dlp-action`, `--dlp-profiles`, `--mask-data-inline`, `--db-security-{create,read,update,delete}`, `--inline-timeout-action`, `--max-inline-latency`, `--mask-data-in-storage`, `--no-active`. Hidden `--config <path>` legacy escape hatch.
    - `update` uses read-modify-write: fetches current profile → merges only specified flags → PUTs full payload. Same protection flags as create. Topic-guardrails never modified by CLI flags. Hidden `--config <path>` legacy escape hatch.
    - `delete` supports `--force --updated-by`
    - `cleanup` deletes old profile revisions, keeps only latest per name. `--force` to proceed, `--updated-by` defaults to git email, `--output json` for structured output. Pure dedup logic in `src/cli/commands/profiles-cleanup.ts`.
    - Profile builder: `src/cli/builders/profile-builder.ts` — `buildProfileRequest()` (create), `buildProfileOverrides()` (update), `mergeProfilePolicy()` (deep merge). Arrays merge by `name` field; objects overlay specified fields.
  - `airs runtime topics {list,get,create,update,delete,apply,eval,revert}` — custom topic CRUD + agent-driven topic commands (supports `--force --updated-by`)
  - `airs runtime api-keys {list,create,regenerate,delete}` — API key management (`regenerate` takes `--interval`/`--unit`)
  - `airs runtime customer-apps {list,get,update,delete,consumption}` — customer app CRUD + `consumption` (per-app token usage + violation breakdown from SCM dashboard; `--time-interval 7|30|60`)
  - `airs runtime deployment-profiles {list}` — deployment profile listing (`--unactivated` filter)
  - `airs runtime scan-logs {query}` — scan log querying (`--interval`/`--unit hours`/`--filter`/`--limit`/`--offset`)
  - `airs runtime dlp filtering-profiles {list, get, replace}` — read + full-replace
  - `airs runtime dlp patterns {list, create, get, replace, patch, delete}` — full CRUD + soft-delete
  - `airs runtime dlp profiles {list, create, get, replace, patch, delete*}` — no real delete; patch profile_status
  - `airs runtime dlp dictionaries {list, create, get, replace, patch, delete}` — multipart upload, 200/204 fallback
  - `airs runtime dlp generate` — generate clean + dirty DLP test files (synthetic sensitive data) across PDF/PNG/JPEG/SVG/DOCX; no auth (local only)

### Red Team (`src/airs/redteam.ts`, `src/airs/promptsets.ts`)
- `SdkRedTeamService` wraps `RedTeamClient` for scan CRUD, polling, reports, **target CRUD**
- `SdkPromptSetService` wraps `RedTeamClient.customAttacks` for prompt set CRUD, prompt CRUD, CSV upload, properties
- 3 scan types: STATIC (attack library), DYNAMIC (agent-driven), CUSTOM (prompt sets)
- `custom_prompt_sets` must be an array of UUID strings (not `{ uuid }` objects) — AIRS API returns 422 otherwise
- ASR/score/threatRate from AIRS API are percentages (0-100), not ratios — render directly, don't multiply by 100
- `listCustomAttacks()` uses `customAttackReports.listCustomAttacks()` for prompt-level results on CUSTOM scans
- `waitForCompletion()` polls with configurable interval, throws on FAILED
- Target create/update accept `{ validate: true }` to validate connection before saving (SDK v0.6.0)
- CLI top-level commands: `scan`, `status <jobId>`, `report <jobId>`, `list`, `abort <jobId>`, `categories`, `languages` (tenant languages; `--management` for mgmt plane)
- CLI subcommand groups: `targets {list,get,create,update,delete,probe,profile,update-profile,validate-auth,metadata,init,templates,backup,restore,error-logs}`, `network-broker {channels {list,get,create,update}, stats}` (channels on distinct `PANW_RED_TEAM_NETWORK_BROKER_ENDPOINT`), `prompt-sets {list,get,create,update,archive,download,upload}`, `prompts {list,get,add,update,delete}`, `properties {list,create,values,add-value}`, `eula`, `instances`, `devices`, `registry-credentials`

### DLP (`src/airs/dlp/`)
- **Shape**: thin SDK wrappers; one class per resource (filtering-profiles, patterns, profiles, dictionaries); all instantiate via `getOrCreateManagementClient()` for shared OAuth token cache
- **Merge-patch semantics**: JSON Merge Patch (RFC 7396) — `null` clears, omit means leave alone. CLI `patch` exposes `--set k=v` (with coercion of `"true"/"false"/numbers/JSON literals`; quote `'"5"'` to force string) and `--clear key` (sets `null`). `--body-file` for nested fields; mutually exclusive with `--set/--clear`.
- **Multipart upload (dictionaries only)**: `create`/`replace` send `json` (metadata) + `file` parts. `--file` required; metadata via flags OR `--metadata-file`.
- **200/204 replace fallback (dictionaries only)**: PUT can return 200 with body or 204 No Content (region-dependent). On 204 the SDK re-GETs; if that fails, the service returns `{ kind: 'fallback', id }` sentinel and the CLI prints `(state not echoed by region)`.
- **No-DELETE for filtering-profiles and profiles**: API doesn't expose DELETE for either. `profiles delete <id>` is a stub that prints the patch idiom and exits 2. `filtering-profiles` has no `delete` subcommand at all.

### Model Security (`src/airs/modelsecurity.ts`)
- `SdkModelSecurityService` wraps `ModelSecurityClient` for security groups, rules, scans, labels, PyPI auth
- snake_case (SDK) → camelCase normalization via `normalizeGroup()`, `normalizeRule()`, etc.
- CLI: `airs model-security {groups,install,labels,models,pypi-auth,rule-instances,rules,scans}`
- Models: read-only catalog browsing — `models {list,get,versions <modelUuid>,version <uuid>,files <modelVersionUuid>}`
- `install` auto-detects uv (uses `uv init` + `uv add`) or falls back to `python3 -m venv` + `pip install`
- Groups: CRUD per source type (LOCAL, S3, GCS, AZURE, HUGGING_FACE)
- Rule instances: state = BLOCKING | ALLOWING | DISABLED
- Scans: create/list/get with evaluations, violations, files sub-queries

### Config (`src/config/`)
- Priority: CLI flags > env vars > `~/.prisma-airs/config.json` > Zod defaults
- All fields in `ConfigSchema` with coercion + defaults; `~` expanded via `expandHome()`

## AIRS Constraints (`src/core/constraints.ts`)

- Topic name: 100 bytes (UTF-8) max
- Description: 250 bytes (UTF-8) max
- Each example: 250 bytes (UTF-8) max, 5 examples max
- Combined (desc + all examples): 1000 bytes (UTF-8) max

## Critical Details

- `scanConcurrency` default 5 — higher risks rate limiting
- `topics create` validates and rejects descriptions exceeding 250 bytes (UTF-8) rather than silently truncating

## Environment Variables

See `.env.example` for the full list. Config priority: CLI flags > env vars > `~/.prisma-airs/config.json` > Zod defaults.

### Required

| Variable | Purpose |
|----------|---------|
| `PANW_AI_SEC_API_KEY` | Prisma AIRS Scanner API |
| `PANW_MGMT_CLIENT_ID` | Prisma AIRS Management OAuth2 |
| `PANW_MGMT_CLIENT_SECRET` | Prisma AIRS Management OAuth2 |
| `PANW_MGMT_TSG_ID` | Prisma AIRS Tenant Service Group |

### Optional

| Variable | Default | Purpose |
|----------|---------|---------|
| `PANW_MGMT_ENDPOINT` | SDK default | Management API endpoint |
| `PANW_MGMT_TOKEN_ENDPOINT` | SDK default | Management API token endpoint |
| `SCAN_CONCURRENCY` | `5` | Concurrent AIRS scans (1-20) |
| `DATA_DIR` | `~/.prisma-airs/runs` | Run state persistence directory |

## Guardrail Optimization Loop

Guardrail generation is **agent-driven** via the atomic `runtime topics` commands — there is no LLM layer and no auto-generation command. An external agent orchestrates the loop:

1. `topics sample` — see the expected eval CSV format (`prompt,expected,intent`).
2. `topics create` — create/update a custom topic (upserts by name; validates AIRS constraints).
3. `topics apply` — assign the topic to a profile (additive, preserves existing topics).
4. `topics eval` — scan a static prompt set against the profile; compute TPR/TNR/coverage/F1 and return FP/FN lists.
5. Keep or `topics revert` (remove from profile + delete the topic) based on metrics, then iterate.

Use `topics get --output json` to read current topic state before modifying. The full protocol (setup, baseline, iteration, plateau detection, companion topics) lives in the docs under `docs-site/docs/runtime/guardrails/` (online: <https://cdot65.github.io/prisma-airs-cli/runtime/guardrails/overview/>).
