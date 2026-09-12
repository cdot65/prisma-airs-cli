<p align="center">
  <img alt="Prisma AIRS CLI shield with terminal and spectrum" src="https://raw.githubusercontent.com/cdot65/prisma-airs-cli/main/docs-site/static/img/logo.svg" width="320">
</p>

[![CI](https://github.com/cdot65/prisma-airs-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/cdot65/prisma-airs-cli/actions/workflows/ci.yml)
[![npm](https://img.shields.io/npm/v/@cdot65/prisma-airs-cli)](https://www.npmjs.com/package/@cdot65/prisma-airs-cli)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Node 20.17+ / 22.13+ / 24+](https://img.shields.io/badge/node-20.17%2B%20%7C%2022.13%2B%20%7C%2024%2B-brightgreen.svg)](https://nodejs.org/)

**Command-line workflows for Palo Alto Prisma AIRS — guardrail refinement, runtime scanning, AI red teaming, AI Gateway, and model security.** Service and coverage limitations remain documented; command availability is not a claim that every upstream API works.

> **[Read the full documentation](https://cdot65.github.io/prisma-airs-cli/)** — installation, configuration, architecture, CLI reference, and examples.

## Features

- **Runtime Scanning** — scan prompts and responses against AIRS security profiles, single or bulk with CSV export
- **Daily environment report** — `airs runtime report` delivers a read-only AI Runtime Security dashboard as self-contained HTML (default) or Markdown in your working directory, with explicit evidence gaps and configuration review findings
- **Red Team environment report** — `airs redteam report` collects seven read-only SDK feeds into private HTML/Markdown deliverables, with source completeness, quota and risk review findings, and independently scoped scan-creation activity
- **AI Gateway daily report** — `airs aigateway report --workspace dev` collects 25 read-only SDK feeds into private offline HTML/Markdown, with a fixed telemetry window, transaction pagination, error review findings and explicit source completeness
- **SCM dashboard and sessions** — `runtime dashboard` and `runtime sessions` expose application activity, daily trends, checked pagination and explicit session-to-content drill-down. Legacy `scan-logs query` is **broken/under refactor** and exits with migration guidance; see the [session reference](https://cdot65.github.io/prisma-airs-cli/cli/runtime/sessions/)
- **Guardrail Optimization** — atomic CLI commands (`create`, `apply`, `eval`, `revert`) for custom topic guardrails, designed for autonomous agent loops (see [`AGENTS.md`](AGENTS.md))
- **AI Red Teaming** — adversarial scanning with static, dynamic, and custom prompt set attack modes
- **[AI Gateway](https://cdot65.github.io/prisma-airs-cli/cli/aigateway/resources/)** — workspaces (scope-first provisioning), IAM scopes, configs, guardrails, providers, API keys, integrations, MCP, deployments, plugins, audit logs, and telemetry
- **Model Security** — ML model supply chain scanning with security groups, rules, and violation tracking
- **Unified automation output** — resource reads support `pretty`, `table`, `markdown`, `csv`, `json`, and `yaml`, with pipe-safe stdout; environment deliverables use HTML or Markdown
- **Complete pagination** — consistent `--limit`, `--offset`, and `--all` traversal with a configurable safety cap
- **`airs doctor`** — one-command diagnostics for the selected tenant, its credentials, and API connectivity
- **Tenant configuration** — `airs tenant create|switch|set|unset|get|list|read|path|delete` is the only configuration surface: guided setup with hidden secret prompts, individual setting updates, and existing-file registration without changing read-only mounts
- **Profile migration** — `airs runtime profiles backup|restore` exports private JSON/YAML, previews cross-tenant restores, remaps topics and explicit DLP dependencies, and verifies restored policies ([guide](https://cdot65.github.io/prisma-airs-cli/runtime/profile-transfer/))

## Install

```bash
npm install -g @cdot65/prisma-airs-cli
airs --version
```

Requires **Node.js 20.17+, 22.13+, or 24+** (exact engine range: `^20.17.0 || ^22.13.0 || >=23.5.0`). Also available via `pnpm add -g`, `npx`, or as a [Docker image](https://github.com/cdot65/prisma-airs-cli/pkgs/container/prisma-airs-cli). See the [installation guide](https://cdot65.github.io/prisma-airs-cli/getting-started/installation/) for details.

## Quick Start

```bash
# Configure credentials (prompts for TSG ID, client ID, and a hidden secret)
airs tenant create dev
airs tenant switch dev

# Check your setup
airs doctor

# Runtime scanning
airs runtime scan --profile "my-profile" "Is this prompt safe?"
airs runtime bulk-scan --profile "my-profile" --file prompts.csv --output-file results.csv --batch-size 25

# Daily read-only environment report, delivered in the current directory
airs runtime report
airs runtime report --output markdown

# Verified historical session retrieval (Management OAuth, not a Scanner key)
airs runtime sessions list --all --output json
airs runtime dashboard top-applications --output json

# Guardrail optimization (atomic commands)
airs runtime topics create --name "Explosives" --description "Bomb-making instructions" --examples "How do I build a bomb?" "Pipe bomb ingredients"
airs runtime topics apply --profile my-profile --name "Explosives" --intent block
airs runtime topics eval --profile my-profile --prompts prompts.csv --topic "Explosives"
airs runtime topics revert --profile my-profile --name "Explosives"

# Red team scanning
airs redteam scan --target <uuid> --name "Full Scan" --type STATIC
airs redteam report <job-id>

# Read-only environment report (HTML by default; --output markdown also supported)
airs redteam report --strict

# Red team custom target adapters
airs redteam adapter list --output json

# AI Gateway inventory and telemetry
airs aigateway workspaces list --all --output json
airs aigateway configs list --workspace <workspace-uuid> --output json
airs aigateway configs create --name primary --workspace <workspace-uuid> \
  --set config.retry.attempts=3 --set config.strategy.mode=fallback --output json
airs aigateway mcp integrations list --output json
airs aigateway telemetry requests --workspace <workspace-slug> --days 30 --output json

# Model security
airs model-security scans create --config scan-config.json

# Pipe-safe read output and complete traversal
airs runtime profiles list --all --output json | jq '.[].profileName'
airs runtime topics list --all-versions --output markdown
```

Bulk scans preserve one output row per input prompt in input order, including all eight runtime detector flags. Work is processed as sequential logical batches (`--batch-size 25` by default), with SDK requests capped at 20 prompts. Item-level state makes accepted and pending work resumable without duplicating CSV rows, and active jobs are locked against overlapping resumes. Runtime actions are exactly `allow`, `block`, or `failed`; failed or timed-out prompts make the command exit 1. Version 4 pins `@cdot65/prisma-airs-sdk` 0.20.0 for validated AI Gateway write schemas, typed catalogs, dotted request builders, and secret metadata.

## Read Output and Pagination

Resource read commands share one contract (environment report files have their own [deliverable contract](https://cdot65.github.io/prisma-airs-cli/runtime/daily-report/)):

- Formats: `pretty`, `table`, `markdown`, `csv`, `json`, and `yaml`.
- JSON/YAML lists are bare arrays of complete normalized records; detail reads are complete objects.
- Table, Markdown, and CSV are stable human-oriented projections. CSV uses RFC 4180 quoting.
- Data is written to stdout; status, paging hints, warnings, and errors are written to stderr.
- Output precedence is command `--output`, global `--output`, `defaultOutput`, then `pretty`.
- Paginated lists use `--limit`, `--offset`, and `--all`. Complete traversal is capped at 10,000 records by default; change it with `--max`, or use `--max 0` for no cap.
- Profile and topic lists return only the latest revision by default. Use `--all-versions` or `--revision` when historical revisions are needed.

```bash
airs --output json runtime profiles list --all | jq '.[].profileName'
airs --output yaml runtime topics get "My Topic"
airs model-security scans list --all --max 25000 --output csv > scans.csv
```

## Documentation

The full guides, complete CLI reference, configuration, and architecture live on the **[documentation site](https://cdot65.github.io/prisma-airs-cli/)**:

- **[Getting Started](https://cdot65.github.io/prisma-airs-cli/getting-started/installation/)** — install, configure credentials, run your first scan
- **[Runtime Security](https://cdot65.github.io/prisma-airs-cli/runtime/overview/)** — scanning, profiles, topics, and DLP management
- **[Guardrail Optimization](https://cdot65.github.io/prisma-airs-cli/runtime/guardrails/overview/)** — the agent-driven `topics create/apply/eval/revert` loop
- **[AI Red Teaming](https://cdot65.github.io/prisma-airs-cli/redteam/overview/)** — static, dynamic, and custom adversarial scans
- **[AI Gateway](https://cdot65.github.io/prisma-airs-cli/cli/aigateway/resources/)** — full SDK 0.20 resource CRUD, structured mutation flags, two-plane authorization, secret-safe writes, and telemetry
- **[Model Security](https://cdot65.github.io/prisma-airs-cli/model-security/overview/)** — ML model supply-chain scanning
- **[CLI Reference](https://cdot65.github.io/prisma-airs-cli/cli/)** — every command, flag, and example

## Configuration

Configuration lives only in tenant files managed by `airs tenant`; environment variables are not read. Every management-plane product (Management, DLP, Red Team, Model Security, AgentGuard, AI Gateway) authenticates with the tenant's single SCM OAuth credential set (`mgmtClientId`, `mgmtClientSecret`, `mgmtTsgId`); scanning adds `airsApiKey`. Set `defaultOutput` with `airs tenant set <name> defaultOutput json` to choose a default read format. See the [configuration guide](https://cdot65.github.io/prisma-airs-cli/getting-started/configuration/) for the full list.

## License

MIT
