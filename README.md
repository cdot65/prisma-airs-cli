# Prisma AIRS CLI

[![CI](https://github.com/cdot65/prisma-airs-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/cdot65/prisma-airs-cli/actions/workflows/ci.yml)
[![npm](https://img.shields.io/npm/v/@cdot65/prisma-airs-cli)](https://www.npmjs.com/package/@cdot65/prisma-airs-cli)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Node 20+](https://img.shields.io/badge/node-%3E%3D20-brightgreen.svg)](https://nodejs.org/)

**Full operational coverage over Palo Alto Prisma AIRS AI security — guardrail refinement, runtime scanning, AI red teaming, model security, and profile audits.**

> **[Read the full documentation](https://cdot65.github.io/prisma-airs-cli/)** — installation, configuration, architecture, CLI reference, and examples.

## Features

- **Runtime Scanning** — scan prompts and responses against AIRS security profiles, single or bulk with CSV export
- **Guardrail Optimization** — atomic CLI commands (`create`, `apply`, `eval`, `revert`) for custom topic guardrails, designed for autonomous agent loops (see `program.md`)
- **AI Red Teaming** — adversarial scanning with static, dynamic, and custom prompt set attack modes
- **Model Security** — ML model supply chain scanning with security groups, rules, and violation tracking
- **Profile Audits** — multi-topic evaluation with per-topic metrics and cross-topic conflict detection

## Install

```bash
npm install -g @cdot65/prisma-airs-cli
airs --version
```

Requires **Node.js >= 20**. Also available via `pnpm add -g`, `npx`, or as a [Docker image](https://github.com/cdot65/prisma-airs-cli/pkgs/container/prisma-airs-cli). See the [installation guide](https://cdot65.github.io/prisma-airs-cli/getting-started/installation/) for details.

## Quick Start

```bash
# Configure credentials
cp .env.example .env   # add your API keys

# Runtime scanning
airs runtime scan --profile "my-profile" "Is this prompt safe?"
airs runtime bulk-scan --profile "my-profile" --input prompts.csv --output results.csv

# Guardrail optimization (atomic commands)
airs runtime topics create --topic "Block bomb-making" --intent block
airs runtime topics apply --profile my-profile --topic "Block bomb-making"
airs runtime topics eval --profile my-profile --input prompts.csv
airs runtime topics revert --profile my-profile --topic "Block bomb-making"

# Red team scanning
airs redteam scan --target <uuid> --name "Full Scan" --type STATIC
airs redteam report <job-id>

# Model security
airs model-security scans create --config scan-config.json
```

## Commands

| Command | Description |
|---------|-------------|
| `runtime scan` | Single prompt scanning against AIRS profiles |
| `runtime bulk-scan` | Batch prompt scanning with CSV output |
| `runtime topics` | Custom topic management (`list`, `get`, `create`, `apply`, `eval`, `revert`, `update`, `delete`) |
| `runtime profiles` | Security profile CRUD (`list`, `get`, `create`, `update`, `delete`) + multi-topic `audit` |
| `runtime api-keys` | API key management |
| `runtime customer-apps` | Customer app CRUD |
| `runtime deployment-profiles` | Deployment profile listing |
| `runtime dlp-profiles` | DLP profile listing |
| `runtime scan-logs` | Scan log querying |
| `redteam scan` | Adversarial scanning (STATIC, DYNAMIC, CUSTOM) |
| `redteam targets` | Red team target CRUD |
| `redteam prompt-sets` | Custom prompt set management |
| `model-security groups` | Security group CRUD |
| `model-security rules` | Security rule management |
| `model-security scans` | Model security scanning |

## Configuration

Credentials are configured via environment variables or `~/.prisma-airs/config.json`. See [`.env.example`](.env.example) for the full list.

**Required for scanning:** `PANW_AI_SEC_API_KEY`
**Required for management:** `PANW_MGMT_CLIENT_ID`, `PANW_MGMT_CLIENT_SECRET`, `PANW_MGMT_TSG_ID`
**Required for profile audits:** one LLM provider key + scanning + management credentials

## License

MIT
