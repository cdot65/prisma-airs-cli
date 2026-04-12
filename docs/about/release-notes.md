# Release Notes

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
