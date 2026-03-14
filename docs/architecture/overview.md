# Architecture Overview

Prisma AIRS CLI is a multi-capability CLI tool and library for Palo Alto Prisma AIRS. Each subsystem has a single responsibility and communicates through typed interfaces — the CLI layer orchestrates user interaction while service layers handle AIRS API communication.

## Module Structure

```
src/
├── cli/              Commands, interactive prompts, terminal rendering
├── config/           Zod-validated config schema + cascade loader
├── core/             Async generator loop, efficacy metrics, AIRS constraints
├── llm/              LangChain provider factory, structured output, prompts
├── airs/             Scanner, Runtime, Management, Red Team, Prompt Sets, Model Security
├── memory/           Learning store, extractor, budget-aware injector
├── persistence/      JSON file store for run state
├── audit/            Profile-level multi-topic evaluation + conflict detection
├── report/           Structured evaluation reports (JSON/HTML)
└── index.ts          Library re-exports
```

## Capability Domains

Prisma AIRS CLI provides five capability domains, each backed by dedicated service and CLI layers:

```mermaid
graph LR
    CLI[CLI Layer] --> GEN[Guardrail Generation]
    CLI --> RT[Runtime Security]
    CLI --> RED[AI Red Teaming]
    CLI --> MS[Model Security]
    CLI --> AUD[Profile Audits]

    GEN --> AIRS_MGMT[AIRS Management API]
    GEN --> AIRS_SCAN[AIRS Scan API]
    GEN --> LLM[LLM Providers]
    RT --> AIRS_SCAN
    RED --> AIRS_RED[AIRS Red Team API]
    MS --> AIRS_MS[AIRS Model Security API]
    AUD --> AIRS_SCAN
    AUD --> AIRS_MGMT
    AUD --> LLM
```

| Domain | CLI Commands | Service Layer |
|--------|-------------|---------------|
| **Guardrail Generation** | `generate`, `resume`, `report`, `list` | Core loop + LLM + Scanner + Management |
| **Runtime Security** | `runtime scan`, `runtime bulk-scan`, `runtime profiles`, `runtime topics`, `runtime api-keys`, `runtime customer-apps`, `runtime deployment-profiles`, `runtime dlp-profiles`, `runtime scan-logs` | `SdkRuntimeService` (sync + async scan) + `SdkManagementService` (config CRUD) |
| **AI Red Teaming** | `redteam scan`, `redteam targets`, `redteam prompt-sets`, `redteam prompts`, `redteam properties` | `SdkRedTeamService` + `SdkPromptSetService` |
| **Model Security** | `model-security groups`, `model-security rules`, `model-security scans`, `model-security labels` | `SdkModelSecurityService` |
| **Profile Audits** | `audit` | Audit runner + Scanner + LLM |

## Guardrail Generation Data Flow

The guardrail generation loop (`airs generate`) is the most complex flow:

```mermaid
graph TD
    A[User Input] --> B[Core Loop]
    B --> C{First iteration?}
    C -->|Yes| D[LLM: Generate Topic]
    C -->|No| E[LLM: Improve Topic]
    D --> F[Deploy to AIRS]
    E --> F
    F --> G[Wait for Propagation]
    G --> H[LLM: Generate Tests]
    H --> I[Scan Test Prompts]
    I --> J[Compute Metrics]
    J --> K[LLM: Analyze FP/FN]
    K --> L{Coverage met?}
    L -->|No| B
    L -->|Yes| M[Extract Learnings]
    M --> N[Save Run State]
```

!!! info "Propagation delay"
    After deploying a topic, Prisma AIRS CLI waits a configurable delay (default 10s) before scanning. AIRS needs this time to propagate changes.

## Runtime Security Data Flow

```mermaid
graph TD
    S1[Single Prompt] --> SYNC[Sync Scan API]
    SYNC --> V1[Verdict: action, category, detections]

    S2[Bulk Prompts File] --> BATCH[Batch into groups of 5]
    BATCH --> ASYNC[Async Scan API]
    ASYNC --> POLL[Poll for completion]
    POLL --> CSV[Write results CSV]
```

## Modules at a Glance

| Module | What it does |
|--------|-------------|
| **`cli/`** | Commander CLI with 8 command groups (`generate`, `resume`, `report`, `list`, `runtime`, `audit`, `redteam`, `model-security`), Inquirer prompts, and Chalk terminal output |
| **`config/`** | Zod schema with coercion and defaults; cascade loader merges CLI flags, env vars, config file, and defaults |
| **`core/`** | AsyncGenerator loop that yields typed events, metric computation (TPR/TNR/F1), and AIRS constraint validation |
| **`llm/`** | Factory for 6 LangChain providers, structured output with Zod schemas, and prompt templates for all 4 LLM calls |
| **`airs/`** | Scanner (sync scan + batched concurrency), Runtime (sync + async bulk scan with polling), Management (topic CRUD, profile CRUD, API keys, customer apps, deployment/DLP profiles, scan logs), Red Team (scan CRUD/polling/reports), Prompt Sets (custom prompt set management), Model Security (groups/rules/scans) |
| **`memory/`** | File-based learning store, LLM-driven extraction after each run, and budget-aware injection into future prompts |
| **`persistence/`** | `JsonFileStore` serializes `RunState` to `~/.prisma-airs/runs/` for pause/resume support |
| **`audit/`** | Profile-level multi-topic evaluation — generates tests per topic, computes per-topic and composite metrics, detects cross-topic conflicts |
| **`report/`** | Structured evaluation report generation — JSON and self-contained HTML output with iteration trends, metrics, and test details |

## Tech Stack

| Category | Technology |
|----------|-----------|
| Language | TypeScript ESM, Node 20+ |
| Package Manager | pnpm |
| LLM Integration | LangChain.js with structured output (Zod schemas) |
| AIRS SDK | `@cdot65/prisma-airs-sdk` |
| CLI | Commander.js + Inquirer + Chalk |
| Testing | Vitest + MSW (fully offline) |
| Lint / Format | Biome |

!!! note "Supported LLM Providers"
    Six providers out of the box: `claude-api` (default), `claude-vertex`, `claude-bedrock`, `gemini-api`, `gemini-vertex`, `gemini-bedrock`. Default model: `claude-opus-4-6`.
