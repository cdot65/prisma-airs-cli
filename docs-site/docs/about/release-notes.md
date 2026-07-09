# Release Notes

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
