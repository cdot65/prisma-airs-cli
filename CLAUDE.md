# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Summary

Prisma AIRS CLI (`airs`) is a CLI and library providing full operational coverage over **Palo Alto Prisma AIRS** AI security capabilities: runtime prompt scanning and configuration management, atomic topic commands (create, apply, eval, revert) for agent-driven optimization following the autoresearch pattern, adversarial red team scanning, ML model supply chain security, and multi-topic profile audits with conflict detection.

## Commands

```bash
# Dev
pnpm install               # Install deps
pnpm run build             # tsc compile to dist/
pnpm run dev               # Run CLI via tsx (any subcommand)

# Test
pnpm test                  # All tests (vitest run)
pnpm test:watch            # Watch mode
pnpm test -- tests/unit/core/metrics.spec.ts   # Single file
pnpm test -- -t "pattern"  # Tests matching name pattern
pnpm test:coverage         # Coverage (excludes src/cli/**, src/index.ts, **/types.ts)
pnpm test:e2e              # E2E tests (requires real creds, opt-in)

# Docker
pnpm run docker:build      # Build local image
pnpm run docker:run        # Run (mounts ~/.prisma-airs)

# Lint & Format
pnpm run lint              # Biome check
pnpm run lint:fix          # Biome check --write
pnpm run format            # Biome format --write
pnpm run format:check      # Biome format (check only, no write)

# Type-check
pnpm tsc --noEmit
```

## Agent-Driven Optimization

The `topics create/apply/eval/revert` commands are designed for autonomous agent loops (autoresearch pattern). An agent can: create a topic, apply it to a profile, eval against a static prompt set, keep or revert based on FP/FN metrics, and repeat. See `AGENTS.md` for the agent loop protocol.

## Code Style (Biome)

Single quotes, semicolons, 2-space indent, 100-char line width. Imports auto-organized.

## Coverage Thresholds

Lines: 90%, Functions: 95%, Branches: 80%, Statements: 90%. Coverage excludes `src/cli/**`, `src/index.ts`, `**/types.ts`.

## Directory Structure

```
src/
├── cli/                   # CLI entry, 3 top-level command groups, prompts, renderer
│   ├── index.ts           # Commander program — registers runtime/redteam/model-security, --debug global flag
│   ├── debug-logger.ts    # Global fetch interceptor — logs AIRS/SCM API traffic to JSONL
│   ├── builders/
│   │   └── profile-builder.ts # CLI flags → CreateSecurityProfileRequest builder + merge utility
│   ├── commands/
│   │   ├── topics-create.ts  # Create or update a custom topic (validates constraints, upserts by name)
│   │   ├── topics-apply.ts   # Assign topic to profile (additive, preserves existing topics)
│   │   ├── topics-eval.ts    # Scan static prompt set, compute metrics, return FP/FN lists
│   │   ├── topics-revert.ts  # Remove topic from profile and delete it
│   │   ├── profiles-cleanup.ts # Delete old profile revisions, keep only latest per name
│   │   ├── runtime.ts     # Runtime scanning + config management + topics + audit (profiles)
│   │   ├── audit.ts       # Profile-level multi-topic evaluation (registered under runtime profiles)
│   │   ├── redteam.ts     # Red team operations (scan, targets CRUD, prompt-sets CRUD, prompts CRUD, properties)
│   │   └── modelsecurity.ts # Model security operations (groups, rules, rule-instances, scans, labels, pypi-auth)
│   ├── bulk-scan-state.ts # Save/load bulk scan IDs for resume after poll failure
│   ├── parse-input.ts     # Input file parsing — CSV (prompt column) or plain text (line-per-prompt)
│   ├── prompts.ts         # Inquirer interactive input collection
│   └── renderer/          # Terminal output (chalk), split by command group
│       ├── index.ts       # Barrel re-exports
│       ├── common.ts      # renderError
│       ├── eval.ts        # Eval metrics, FP/FN list rendering
│       ├── redteam.ts     # Red team scan/target/prompt-set rendering
│       ├── runtime.ts     # Runtime scan + config management rendering
│       ├── audit.ts       # Audit topics, results, conflicts
│       └── modelsecurity.ts # Model security groups/rules/scans rendering
├── config/
│   ├── schema.ts          # Zod ConfigSchema — all config fields w/ defaults
│   └── loader.ts          # Config cascade: CLI > env > file > Zod defaults
├── core/
│   ├── prompt-loader.ts   # Load static prompt set from CSV/text for eval command
│   ├── types.ts           # CustomTopic, EvalMetrics, EvalResult, TopicConstraints
│   ├── metrics.ts         # computeMetrics() — TP/TN/FP/FN → TPR/TNR/accuracy/coverage/F1
│   └── constraints.ts     # AIRS topic limits: 100 name, 250 desc, 250/example, 5 max, 1000 combined
├── llm/
│   ├── provider.ts        # createLlmProvider() — factory for LangChain providers (used by audit)
│   ├── service.ts         # LangChainLlmService
│   ├── schemas.ts         # Zod output schemas for structured LLM responses
│   └── prompts/           # ChatPromptTemplate definitions
├── airs/
│   ├── scanner.ts         # AirsScanService + DebugScanService + RateLimitedScanService — syncScan + scanBatch
│   ├── runtime.ts         # SdkRuntimeService — sync scan, async bulk scan, poll results, CSV export
│   ├── management.ts      # SdkManagementService — topic CRUD, profile CRUD, API keys, customer apps, deployment/DLP profiles, scan logs
│   ├── promptsets.ts      # SdkPromptSetService — custom prompt set CRUD via RedTeamClient
│   ├── redteam.ts         # SdkRedTeamService — red team scan CRUD, polling, reports
│   ├── modelsecurity.ts   # SdkModelSecurityService — security groups, rules, scans, labels
│   └── types.ts           # ScanResult, ScanService, ManagementService, PromptSetService, RedTeamService, ModelSecurityService
├── audit/
│   ├── types.ts           # ProfileTopic, TopicAuditResult, ConflictPair, AuditResult, AuditEvent
│   ├── evaluator.ts       # groupResultsByTopic, computeTopicAuditResults, computeCompositeMetrics, detectConflicts
│   ├── runner.ts          # runAudit() async generator — yields AuditEvent
│   └── report.ts          # buildAuditReportJson(), buildAuditReportHtml()
├── report/
│   ├── types.ts           # ReportOutput, TestDetail, RunDiff, MetricsDelta
│   ├── json.ts            # buildReportJson() — RunState → structured ReportOutput
│   └── html.ts            # buildReportHtml() — ReportOutput → self-contained HTML
└── index.ts               # Library exports

tests/
├── unit/                  # spec files
│   ├── airs/              # scanner.spec.ts, management.spec.ts, modelsecurity.spec.ts, promptsets.spec.ts, redteam.spec.ts, runtime.spec.ts
│   ├── audit/             # evaluator.spec.ts, runner.spec.ts, report.spec.ts
│   ├── cli/               # parse-input.spec.ts, bulk-scan-state.spec.ts
│   ├── config/            # schema.spec.ts, loader.spec.ts
│   ├── core/              # metrics.spec.ts, constraints.spec.ts
│   ├── llm/               # provider.spec.ts, schemas.spec.ts, service.spec.ts, prompts.spec.ts
│   └── report/            # json.spec.ts, html.spec.ts
├── e2e/                   # vertex-provider.e2e.spec.ts (opt-in, requires real creds)
└── helpers/               # mocks.ts
```

## Architecture

### Topic Commands (`src/cli/commands/topics-*.ts`)
- **`create`** (`topics-create.ts`): create or update a custom topic; validates AIRS constraints (name ≤100, desc ≤250, each example ≤250, combined ≤1000, max 5 examples), upserts by name
- **`apply`** (`topics-apply.ts`): assign topic to a security profile; additive — reads current profile topic-list, appends the new topic with correct `revision`, writes back; never clobbers existing topics
- **`eval`** (`topics-eval.ts`): load a static prompt set (CSV or text), scan each prompt against the named profile, compute TP/TN/FP/FN → TPR/TNR/coverage/F1, return FP and FN lists for agent inspection
- **`revert`** (`topics-revert.ts`): remove topic from profile topic-list and delete the topic; safe — checks profile reference before deleting

These four commands compose into an autoresearch-style optimization loop: an agent calls `create → apply → eval`, decides keep or `revert`, then iterates.

### AIRS Integration (`src/airs/`)
- **Scanner**: `Scanner.syncScan()` via SDK, detection = `prompt_detected.topic_violation === true` (sole signal, no fallbacks)
- **Detection**: `triggered` (= `topic_violation`) is the sole guardrail detection signal. No category-based or action-based detection.
- **`DebugScanService`**: Wrapper that appends raw scan responses to a JSONL file when `--debug-scans` is passed
- **`RateLimitedScanService`**: Wrapper that caps scan throughput to N calls/second via sliding-window token bucket
- **`--debug` global flag**: Intercepts `globalThis.fetch` to log all AIRS/SCM API requests and responses to `~/.prisma-airs/debug-api-<timestamp>.jsonl`. Auth tokens are redacted. Works with any subcommand.
- **Prompt sets**: `SdkPromptSetService` wraps `RedTeamClient.customAttacks` for custom prompt set CRUD
- **Management**: `ManagementClient` via OAuth2 — topic CRUD, security profile CRUD, API key management, customer app management, deployment/DLP profile listing, scan log querying
- Profile updates create **new revisions with new UUIDs** — always reference profiles by name, never ID
- Topics must be added to profile's `model-protection` → `topic-guardrails` → `topic-list`
- AIRS rejects empty `topic-list` entries — only include entries with topics
- **CRITICAL: topic-list `revision` field**: AIRS pins topic content to the `revision` number in the profile's topic-list. Omitting it defaults to revision 0 (original creation content). `topics apply` fetches current topic revisions via `listTopics()` and includes them.
- **CRITICAL: always scan by profile NAME**, never by profile ID/UUID. Scanning by name always uses the latest profile version; scanning by ID pins to a versioned snapshot.
- Topics can't be deleted while referenced by any profile revision
- **Platform ceilings**: Topics in high-sensitivity domains (explosives, weapons) trigger built-in AIRS safety that overrides custom definitions. Shorter descriptions generally outperform longer ones with exclusion clauses.

### Runtime Scanning (`src/airs/runtime.ts`)
- `SdkRuntimeService` wraps SDK `Scanner` for sync and async scanning
- `scanPrompt()` — sync scan via `syncScan()`, normalizes to `RuntimeScanResult`
- **Detection scope**: `scanPrompt()` aggregates 6 detection types via OR (`topic_violation`, `injection`, `toxic_content`, `dlp`, `url_cats`, `malicious_code`). This is intentionally broader than the guardrail loop's `topic_violation`-only signal — runtime scanning is a general-purpose firewall check, not topic-specific evaluation.
- `submitBulkScan()` — batches prompts into groups of 5 `AsyncScanObject` items, calls `asyncScan()` per batch; optional `sessionId` for AIRS Sessions UI grouping
- `pollResults()` — sweeps all pending scan IDs in batches of 5 per cycle; retries on rate limit with exponential backoff (10s base); retry level decays by 1 after a full successful sweep (not per-batch); inter-batch and inter-sweep delays scale with rate limit pressure
- `formatResultsCsv()` — static method producing CSV from results
- CLI: `airs runtime scan --profile <name> [--response <text>] <prompt>`
- CLI: `airs runtime bulk-scan --profile <name> --input <file> [--output <file>] [--session-id <id>]`
- Input file parsing: `.csv` files extract the `prompt` column by header; `.txt`/extensionless use line-per-prompt
- Bulk scan IDs are saved to `~/.prisma-airs/bulk-scans/` before polling — survives rate limit crashes
- CLI: `airs runtime resume-poll <stateFile> [--output <file>]` — resume polling from saved scan IDs
- CLI config management subcommand groups (all via `ManagementClient` OAuth2):
  - `airs runtime profiles {list,get,create,update,delete,audit,cleanup}` — security profile CRUD + profile audit + revision cleanup
    - `get` accepts name or UUID, supports `--output pretty|json|yaml`
    - `create` requires `--name`, plus optional protection flags: `--prompt-injection`, `--toxic-content`, `--contextual-grounding`, `--malicious-code`, `--url-action`, `--allow-url-categories`, `--block-url-categories`, `--alert-url-categories`, `--agent-security`, `--dlp-action`, `--dlp-profiles`, `--mask-data-inline`, `--db-security-{create,read,update,delete}`, `--inline-timeout-action`, `--max-inline-latency`, `--mask-data-in-storage`, `--no-active`. Hidden `--config <path>` legacy escape hatch.
    - `update` uses read-modify-write: fetches current profile → merges only specified flags → PUTs full payload. Same protection flags as create. Topic-guardrails never modified by CLI flags. Hidden `--config <path>` legacy escape hatch.
    - `delete` supports `--force --updated-by`
    - `cleanup` deletes old profile revisions, keeps only latest per name. `--force` to proceed, `--updated-by` defaults to git email, `--output json` for structured output. Pure dedup logic in `src/cli/commands/profiles-cleanup.ts`.
    - Profile builder: `src/cli/builders/profile-builder.ts` — `buildProfileRequest()` (create), `buildProfileOverrides()` (update), `mergeProfilePolicy()` (deep merge). Arrays merge by `name` field; objects overlay specified fields.
  - `airs runtime topics {list,get,create,update,delete,apply,eval,revert}` — custom topic CRUD + agent-driven topic commands (supports `--force --updated-by`)
  - `airs runtime api-keys {list,create,regenerate,delete}` — API key management (`regenerate` takes `--interval`/`--unit`)
  - `airs runtime customer-apps {list,get,update,delete}` — customer app CRUD
  - `airs runtime deployment-profiles {list}` — deployment profile listing (`--unactivated` filter)
  - `airs runtime dlp-profiles {list}` — DLP profile listing
  - `airs runtime scan-logs {query}` — scan log querying (`--interval`/`--unit hours`/`--filter`)

### Red Team (`src/airs/redteam.ts`, `src/airs/promptsets.ts`)
- `SdkRedTeamService` wraps `RedTeamClient` for scan CRUD, polling, reports, **target CRUD**
- `SdkPromptSetService` wraps `RedTeamClient.customAttacks` for prompt set CRUD, prompt CRUD, CSV upload, properties
- 3 scan types: STATIC (attack library), DYNAMIC (agent-driven), CUSTOM (prompt sets)
- `custom_prompt_sets` must be an array of UUID strings (not `{ uuid }` objects) — AIRS API returns 422 otherwise
- ASR/score/threatRate from AIRS API are percentages (0-100), not ratios — render directly, don't multiply by 100
- `listCustomAttacks()` uses `customAttackReports.listCustomAttacks()` for prompt-level results on CUSTOM scans
- `waitForCompletion()` polls with configurable interval, throws on FAILED
- Target create/update accept `{ validate: true }` to validate connection before saving (SDK v0.6.0)
- CLI top-level commands: `scan`, `status <jobId>`, `report <jobId>`, `list`, `abort <jobId>`, `categories`
- CLI subcommand groups: `targets {list,get,create,update,delete,probe,profile,update-profile}`, `prompt-sets {list,get,create,update,archive,download,upload}`, `prompts {list,get,add,update,delete}`, `properties {list,create,values,add-value}`

### Model Security (`src/airs/modelsecurity.ts`)
- `SdkModelSecurityService` wraps `ModelSecurityClient` for security groups, rules, scans, labels, PyPI auth
- snake_case (SDK) → camelCase normalization via `normalizeGroup()`, `normalizeRule()`, etc.
- CLI: `airs model-security {groups,install,labels,pypi-auth,rule-instances,rules,scans}`
- `install` auto-detects uv (uses `uv init` + `uv add`) or falls back to `python3 -m venv` + `pip install`
- Groups: CRUD per source type (LOCAL, S3, GCS, AZURE, HUGGING_FACE)
- Rule instances: state = BLOCKING | ALLOWING | DISABLED
- Scans: create/list/get with evaluations, violations, files sub-queries

### LLM Service (`src/llm/`)
- Used by the `audit` command for test generation and analysis
- 6 providers: `claude-api` (default), `claude-vertex`, `claude-bedrock`, `gemini-api`, `gemini-vertex`, `gemini-bedrock`
- Default model: `claude-opus-4-6` (Vertex: `claude-opus-4-6`, Bedrock: `anthropic.claude-opus-4-6-v1`), Gemini providers: `gemini-2.5-pro`
- `claude-vertex` default region: `global` (not `us-central1`)
- Structured output via `withStructuredOutput(ZodSchema)` — 3 retries on parse failure

### Config (`src/config/`)
- Priority: CLI flags > env vars > `~/.prisma-airs/config.json` > Zod defaults
- All fields in `ConfigSchema` with coercion + defaults; `~` expanded via `expandHome()`

### Reports (`src/report/`)
- `buildReportJson(run, opts)` maps `RunState` → `ReportOutput` (pure function, no I/O)
- `buildReportHtml(report)` renders `ReportOutput` → self-contained HTML string
- `--format json|html|terminal`, `--tests` for per-test details, `--diff <runId>` for run comparison
- HTML includes embedded CSS, iteration trends table, metrics, test result tables, diff sections

### Audit (`src/audit/`)
- `runAudit()` async generator yields `AuditEvent` discriminated union: `topics:loaded`, `tests:generated`, `scan:progress`, `evaluate:complete`, `audit:complete`
- Reads all topics from profile via `getProfileTopics()`, generates tests per topic (tagged with `targetTopic`), batch scans, evaluates per-topic + composite metrics
- Both intents use `triggered` (= `prompt_detected.topic_violation`) as sole detection signal
- `detectConflicts()` finds FN/FP overlaps between topic pairs — same prompt failing as FN for topic A and FP for topic B
- `getProfileTopics()` reads profile policy `model-protection → topic-guardrails → topic-list`, cross-references with `listTopics()` for full details

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

### Required (one set per provider)

| Variable | Purpose |
|----------|---------|
| `ANTHROPIC_API_KEY` | Claude API provider |
| `GOOGLE_API_KEY` | Gemini API provider |
| `GOOGLE_CLOUD_PROJECT` | Vertex AI (Claude or Gemini) |
| `GOOGLE_CLOUD_LOCATION` | Vertex AI region (default: `us-central1`, Claude Vertex: `global`) |
| `AWS_REGION` | Bedrock region (default: `us-east-1`) |
| `AWS_ACCESS_KEY_ID` | Bedrock auth |
| `AWS_SECRET_ACCESS_KEY` | Bedrock auth |
| `PANW_AI_SEC_API_KEY` | Prisma AIRS Scanner API |
| `PANW_MGMT_CLIENT_ID` | Prisma AIRS Management OAuth2 |
| `PANW_MGMT_CLIENT_SECRET` | Prisma AIRS Management OAuth2 |
| `PANW_MGMT_TSG_ID` | Prisma AIRS Tenant Service Group |

### Optional

| Variable | Default | Purpose |
|----------|---------|---------|
| `LLM_PROVIDER` | `claude-api` | LLM provider selection |
| `LLM_MODEL` | per-provider | Override model name |
| `PANW_MGMT_ENDPOINT` | SDK default | Management API endpoint |
| `PANW_MGMT_TOKEN_ENDPOINT` | SDK default | Management API token endpoint |
| `SCAN_CONCURRENCY` | `5` | Concurrent AIRS scans (1-20) |
| `DATA_DIR` | `~/.prisma-airs/runs` | Run state persistence directory |

## Guardrail Optimization Loop

For autonomous custom topic guardrail optimization, follow the protocol in `program.md`. It covers setup, baseline, the iteration loop, revert procedure, plateau detection, and companion topics. All lessons from real optimization sessions are encoded there.

Key commands: `topics create`, `topics apply`, `topics eval`, `topics revert`, `topics sample`, `topics get --output json`.
