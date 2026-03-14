# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Summary

Prisma AIRS CLI (`airs`) is a CLI and library providing full operational coverage over **Palo Alto Prisma AIRS** AI security capabilities: runtime prompt scanning and configuration management, LLM-driven guardrail generation with iterative refinement, adversarial red team scanning, ML model supply chain security, and multi-topic profile audits with conflict detection. The guardrail loop uses an LLM to produce topic definitions, deploys to Prisma AIRS, scans test prompts, evaluates efficacy (TPR, TNR, coverage, F1), and improves until a coverage target is met. Cross-run memory persists learnings for future runs.

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

## Tech Stack

TypeScript ESM, Node 20+, pnpm. LangChain.js w/ structured output (Zod). `@cdot65/prisma-airs-sdk` for AIRS scan+management APIs. Commander.js CLI, Inquirer prompts, Chalk rendering. Vitest+MSW tests. Biome lint/format.

## Code Style (Biome)

Single quotes, semicolons, 2-space indent, 100-char line width. Imports auto-organized.

## Coverage Thresholds

Lines: 90%, Functions: 95%, Branches: 80%, Statements: 90%. Coverage excludes `src/cli/**`, `src/index.ts`, `**/types.ts`.

## Directory Structure

```
src/
├── cli/                   # CLI entry, 3 top-level groups + deprecated aliases, prompts, renderer
│   ├── index.ts           # Commander program — registers runtime/redteam/model-security + deprecated top-level aliases
│   ├── commands/
│   │   ├── generate.ts    # Main loop orchestration, wires all services (registered under runtime topics)
│   │   ├── resume.ts      # Resume paused/failed run from disk (registered under runtime topics)
│   │   ├── report.ts      # View run results by ID (registered under runtime topics)
│   │   ├── list.ts        # List all saved runs (registered as "runs" under runtime topics)
│   │   ├── runtime.ts     # Runtime scanning + config management + guardrail generation (topics) + audit (profiles)
│   │   ├── audit.ts       # Profile-level multi-topic evaluation (registered under runtime profiles)
│   │   ├── redteam.ts     # Red team operations (scan, targets CRUD, prompt-sets CRUD, prompts CRUD, properties)
│   │   └── modelsecurity.ts # Model security operations (groups, rules, rule-instances, scans, labels, pypi-auth)
│   ├── bulk-scan-state.ts # Save/load bulk scan IDs for resume after poll failure
│   ├── parse-input.ts     # Input file parsing — CSV (prompt column) or plain text (line-per-prompt)
│   ├── prompts.ts         # Inquirer interactive input collection
│   └── renderer/          # Terminal output (chalk), split by command group
│       ├── index.ts       # Barrel re-exports
│       ├── common.ts      # renderError
│       ├── generate.ts    # Guardrail loop rendering (header, topic, metrics, analysis)
│       ├── redteam.ts     # Red team scan/target/prompt-set rendering
│       ├── runtime.ts     # Runtime scan + config management rendering
│       ├── audit.ts       # Audit topics, results, conflicts
│       └── modelsecurity.ts # Model security groups/rules/scans rendering
├── config/
│   ├── schema.ts          # Zod ConfigSchema — all config fields w/ defaults
│   └── loader.ts          # Config cascade: CLI > env > file > Zod defaults
├── core/
│   ├── loop.ts            # AsyncGenerator main loop — yields LoopEvent
│   ├── types.ts           # CustomTopic, UserInput, TestCase, TestResult, EfficacyMetrics, AnalysisReport, IterationResult, RunState, LoopEvent
│   ├── metrics.ts         # computeMetrics() — TP/TN/FP/FN → TPR/TNR/accuracy/coverage/F1
│   └── constraints.ts     # AIRS topic limits: 100 name, 250 desc, 250/example, 5 max, 1000 combined
├── llm/
│   ├── provider.ts        # createLlmProvider() — factory for 6 LangChain providers
│   ├── service.ts         # LangChainLlmService — generateTopic, generateTests, improveTopic, analyzeResults
│   ├── schemas.ts         # Zod output schemas for structured LLM responses
│   └── prompts/           # ChatPromptTemplate definitions (6 files)
├── airs/
│   ├── scanner.ts         # AirsScanService + DebugScanService — syncScan + scanBatch
│   ├── runtime.ts         # SdkRuntimeService — sync scan, async bulk scan, poll results, CSV export
│   ├── management.ts      # SdkManagementService — topic CRUD, profile CRUD, API keys, customer apps, deployment/DLP profiles, scan logs
│   ├── promptsets.ts      # SdkPromptSetService — custom prompt set CRUD via RedTeamClient
│   ├── redteam.ts         # SdkRedTeamService — red team scan CRUD, polling, reports
│   ├── modelsecurity.ts   # SdkModelSecurityService — security groups, rules, scans, labels
│   └── types.ts           # ScanResult, ScanService, ManagementService, PromptSetService, RedTeamService, ModelSecurityService
├── memory/
│   ├── store.ts           # MemoryStore — file-based persistence, keyword category matching (≥50% overlap)
│   ├── extractor.ts       # LearningExtractor — post-loop LLM extraction, merge/corroboration
│   ├── injector.ts        # MemoryInjector — budget-aware prompt injection (3000 char default)
│   ├── diff.ts            # computeIterationDiff() — metric deltas between iterations
│   ├── types.ts           # Learning, TopicMemory, IterationDiff
│   ├── schemas.ts         # LearningExtractionOutputSchema
│   └── prompts/
│       └── extract-learnings.ts
├── persistence/
│   ├── store.ts           # JsonFileStore — save/load/list RunState as JSON
│   └── types.ts           # RunStore, RunStateSummary
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
├── unit/                  # 28 spec files
│   ├── airs/              # scanner.spec.ts, management.spec.ts, modelsecurity.spec.ts, promptsets.spec.ts, redteam.spec.ts, runtime.spec.ts
│   ├── audit/             # evaluator.spec.ts, runner.spec.ts, report.spec.ts
│   ├── cli/               # parse-input.spec.ts, bulk-scan-state.spec.ts
│   ├── config/            # schema.spec.ts, loader.spec.ts
│   ├── core/              # loop.spec.ts, metrics.spec.ts, constraints.spec.ts
│   ├── llm/               # provider.spec.ts, schemas.spec.ts, service.spec.ts, prompts.spec.ts
│   ├── memory/            # store.spec.ts, extractor.spec.ts, injector.spec.ts, diff.spec.ts, prompts.spec.ts
│   ├── persistence/       # store.spec.ts
│   └── report/            # json.spec.ts, html.spec.ts
├── integration/           # loop.integration.spec.ts (full loop w/ mocks)
├── e2e/                   # vertex-provider.e2e.spec.ts (opt-in, requires real creds)
└── helpers/               # mocks.ts
```

## Architecture

### Core Loop (`src/core/loop.ts`)
- `runLoop()` async generator yields typed `LoopEvent` discriminated unions
- Events yielded by `runLoop()`: `iteration:start`, `generate:complete`, `companion:generated` (block-intent iter 1), `companion:created` (block-intent iter 1), `apply:complete`, `probe:waiting` (iter 1 only, topic not yet active), `probe:ready` (iter 1 only, topic confirmed active), `tests:composed` (iter 2+, always-on composition), `tests:accumulated` (if accumulation enabled, iter 2+), `test:progress`, `evaluate:complete`, `analyze:complete`, `iteration:complete`, `topic:duplicate` (when improveTopic/simplifyTopic returns identical topic), `topic:reverted` (tier 1 recovery), `topic:simplified` (tier 2 recovery), `loop:plateau` (opt-in plateau detection), `memory:extracted` (if memory enabled), `promptset:created` (if `--create-prompt-set`), `loop:complete`
- Events defined in `LoopEvent` union but **not yielded** by `runLoop()`: `loop:paused` (reserved for future use), `memory:loaded` (emitted by CLI before loop starts)
- `apply:complete` is yielded but intentionally unhandled in CLI commands (no user-facing output needed)
- **Warm-up probe** (iter 1 only): after propagation delay, scans `topic.examples[0]` via `scanner.scan()` in a retry loop (default 6 attempts, 5s interval) to verify topic+profile revision are active before full test suite. Skipped if topic has no examples. Configurable via `LoopDependencies.probeIntervalMs` and `maxProbeAttempts`
- **Two-phase generation** (block-intent only): AIRS needs BOTH allow and block topics sharing the same vocabulary domain. On iter 1, generates a domain-specific allow companion via `LlmService.generateCompanionTopic()` that covers the benign/legitimate side of the same domain (e.g., "Legal Tax Planning" as companion to "Tax Evasion"). Wires both to profile with `guardrailAction='allow'` (default: allow everything, block topic carves out violations). Companion is one-shot (no refinement). Iter 2+ only updates block topic content. Allow-intent runs use `guardrailAction='block'` with a single topic.
- Topic name **locked after iteration 1** — only description+examples change thereafter
- `analyzeResults()` and `improveTopic()` receive intent param — prioritizes FN for block, FP for allow
- **Test composition** (always-on, iter 2+): carried FP/FN failures + regression tier (TP/TN re-scanned) + fresh LLM tests. `TestCase.source` tags each test's origin. `EfficacyMetrics.regressionCount` tracks regression-tier failures.
- **Weighted category generation** (always-on, iter 2+): `computeCategoryBreakdown()` passes per-category error rates to the LLM prompt, biasing test generation toward weak areas
- Optional test accumulation (`accumulateTests`) carries full test pool across iterations with case-insensitive dedup; `maxAccumulatedTests` caps growth
- Stop conditions: `coverage >= targetCoverage` (default 0.9), `consecutiveRegressions >= maxRegressions` (default 3, 0 = disabled), or plateau detection (`--plateau-window`, opt-in). Coverage = `min(TPR, TNR)`
- **3-tier recovery** on consecutive regressions: (1) revert to best-performing topic (no LLM), (2) LLM simplification, (3) early stop. Each tier gets 2 regressions before escalating.
- **Duplicate detection**: `findDuplicateIteration()` compares description+examples against all prior iterations. Duplicates skip scanning, increment regression counter, and trigger recovery tiers.
- **Plateau detection** (opt-in, `--plateau-window N`): if last N iterations are within ±band% without exceeding best, yields `loop:plateau` and stops.
- **Early stopping on regression**: `RunState.consecutiveRegressions` tracks how many consecutive iterations failed to improve `bestCoverage`. Resets to 0 on improvement. `UserInput.maxRegressions` controls the threshold (default 3, 0 disables).
- **Description simplification**: After 2 consecutive regressions, if `hasTriedSimplification` is false and a best iteration exists, the loop calls `simplifyTopic()` to strip exclusion clauses and shorten the description. Resets regression counter to 0. Only attempted once per run (`RunState.hasTriedSimplification`). If simplification also regresses, early stopping kicks in at `maxRegressions`.

### AIRS Integration (`src/airs/`)
- **Scanner**: `Scanner.syncScan()` via SDK, detection = `prompt_detected.topic_violation === true` (sole signal, no fallbacks). `category` still extracted for weighted test generation but not used for detection
- **Detection**: Both block and allow intents use `triggered` (= `topic_violation`) as the sole guardrail detection signal. No category-based or action-based detection.
- **`DebugScanService`**: Wrapper that appends raw scan responses to a JSONL file when `--debug-scans` is passed
- **Prompt sets**: `SdkPromptSetService` wraps `RedTeamClient.customAttacks` for custom prompt set CRUD; `--create-prompt-set` auto-creates a prompt set from the best iteration's test cases
- **Management**: `ManagementClient` via OAuth2 — topic CRUD, security profile CRUD, API key management, customer app management, deployment/DLP profile listing, scan log querying, plus profile linking for guardrail generation
- Profile updates create **new revisions with new UUIDs** — always reference profiles by name, never ID
- Topics must be added to profile's `model-protection` → `topic-guardrails` → `topic-list`
- AIRS rejects empty `topic-list` entries — only include entries with topics (no empty opposite-action entry)
- **Block-intent requires domain-specific allow topic**: AIRS needs BOTH allow and block topics sharing the same vocabulary domain. `assignTopicsToProfile()` wires both with `guardrailAction='allow'` for block-intent (default: allow everything, block topics carve out violations). Allow-intent uses `guardrailAction='block'`.
- **CRITICAL: topic-list `revision` field**: AIRS pins topic content to the `revision` number in the profile's topic-list. Omitting it defaults to revision 0 (original creation content). `assignTopicsToProfile()` fetches current topic revisions via `listTopics()` and includes them.
- **CRITICAL: always scan by profile NAME**, never by profile ID/UUID. Scanning by name always uses the latest profile version; scanning by ID pins to a versioned snapshot.
- Topics can't be deleted while referenced by any profile revision
- **Platform ceilings**: Block-intent topics in high-sensitivity domains (explosives, weapons) trigger built-in AIRS safety that overrides custom definitions (0% TNR). Allow-intent topics use broad semantic matching — exclusion clauses increase FP; shorter descriptions outperform longer ones. Typical block-intent ceiling: 40–50% coverage due to vocabulary overlap between allow and block domains.

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
  - `airs runtime profiles {list,create,update,delete,audit}` — security profile CRUD + profile audit (supports `--force --updated-by`)
  - `airs runtime topics {list,create,update,delete,generate,resume,report,runs}` — custom topic CRUD + guardrail generation (supports `--force --updated-by`)
  - `airs runtime api-keys {list,create,regenerate,delete}` — API key management (`regenerate` takes `--interval`/`--unit`)
  - `airs runtime customer-apps {list,get,update,delete}` — customer app CRUD
  - `airs runtime deployment-profiles {list}` — deployment profile listing (`--unactivated` filter)
  - `airs runtime dlp-profiles {list}` — DLP profile listing
  - `airs runtime scan-logs {query}` — scan log querying (`--interval`/`--unit hours`/`--filter`)
- Deprecated top-level aliases (`airs generate`, `airs resume`, `airs report`, `airs list`, `airs audit`) still work with deprecation warnings

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
- CLI: `airs model-security {groups,rules,rule-instances,scans,labels,pypi-auth}`
- Groups: CRUD per source type (LOCAL, S3, GCS, AZURE, HUGGING_FACE)
- Rule instances: state = BLOCKING | ALLOWING | DISABLED
- Scans: create/list/get with evaluations, violations, files sub-queries

### LLM Service (`src/llm/`)
- 6 providers: `claude-api` (default), `claude-vertex`, `claude-bedrock`, `gemini-api`, `gemini-vertex`, `gemini-bedrock`
- Default model: `claude-opus-4-6` (Vertex: `claude-opus-4-6`, Bedrock: `anthropic.claude-opus-4-6-v1`), Gemini providers: `gemini-2.5-pro`
- `claude-vertex` default region: `global` (not `us-central1`)
- All 6 calls (generateTopic, generateCompanionTopic, generateTests, analyzeResults, improveTopic, simplifyTopic) use `withStructuredOutput(ZodSchema)` — 3 retries on parse failure
- Memory injected via `{memorySection}` template variable
- `clampTopic()` enforces AIRS constraints post-LLM (not Zod) — drops examples, trims description
- `improveTopic()` accepts optional `bestContext` param `{ bestCoverage, bestIteration, bestTopic? }` — injects regression warnings into the prompt when coverage drops below the best iteration, and always shows best-iteration context
- Improve-topic system prompt includes CRITICAL PLATFORM CONSTRAINT section warning against exclusion clauses and favoring shorter descriptions

### Memory System (`src/memory/`)
- File-based at `~/.prisma-airs/memory/{category}.json`
- Category = normalized keyword extraction (stop-word removal, alphabetical sort)
- Cross-topic transfer when keyword overlap ≥ 50%
- Budget-aware injection (3000 char default): sorts by corroboration count desc, verbose→compact→omit

### Config (`src/config/`)
- Priority: CLI flags > env vars > `~/.prisma-airs/config.json` > Zod defaults
- All fields in `ConfigSchema` with coercion + defaults; `~` expanded via `expandHome()`

### Persistence (`src/persistence/`)
- `JsonFileStore` saves/loads `RunState` as JSON at `~/.prisma-airs/runs/{runId}.json`

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

- `propagationDelayMs` default 10s — AIRS needs propagation time after topic create/update
- `scanConcurrency` default 5 — higher risks rate limiting
- LLM description output routinely exceeds 250 char AIRS limit — `clampTopic()` handles this

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
| `PROPAGATION_DELAY_MS` | `10000` | Wait after topic create/update (ms) |
| `ACCUMULATE_TESTS` | `false` | Carry test pool across iterations |
| `MAX_ACCUMULATED_TESTS` | — | Cap on accumulated tests |
| `DATA_DIR` | `~/.prisma-airs/runs` | Run state persistence directory |
| `MEMORY_ENABLED` | `true` | Cross-run learning memory |
| `MEMORY_DIR` | `~/.prisma-airs/memory` | Memory store directory |
| `MAX_MEMORY_CHARS` | `3000` | Memory injection budget (500-10000) |
