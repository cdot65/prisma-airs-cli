# Release Notes

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
