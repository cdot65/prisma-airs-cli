# Prisma AIRS CLI

[![CI](https://github.com/cdot65/prisma-airs-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/cdot65/prisma-airs-cli/actions/workflows/ci.yml)
[![npm](https://img.shields.io/npm/v/@cdot65/prisma-airs-cli)](https://www.npmjs.com/package/@cdot65/prisma-airs-cli)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Node 20+](https://img.shields.io/badge/node-%3E%3D20-brightgreen.svg)](https://nodejs.org/)

**Full operational coverage over Palo Alto Prisma AIRS AI security — guardrail refinement, runtime scanning, AI red teaming, model security, and profile audits.**

> **[Read the full documentation](https://cdot65.github.io/prisma-airs-cli/)** — installation, configuration, architecture, CLI reference, and examples.

## Features

- **Runtime Scanning** — scan prompts and responses against AIRS security profiles, single or bulk with CSV export
- **Guardrail Generation** — LLM-driven iterative refinement loop that generates, deploys, tests, and improves custom topic definitions until a coverage target is met
- **AI Red Teaming** — adversarial scanning with static, dynamic, and custom prompt set attack modes
- **Model Security** — ML model supply chain scanning with security groups, rules, and violation tracking
- **Profile Audits** — multi-topic evaluation with per-topic metrics and cross-topic conflict detection
- **Cross-run Memory** — persists learnings across guardrail generation runs for faster convergence

## Install

```bash
npm install -g @cdot65/prisma-airs-cli
```

Requires **Node.js >= 20**. Also available as a [Docker image](https://github.com/cdot65/prisma-airs-cli/pkgs/container/prisma-airs-cli).

## Quick Start

```bash
# Configure credentials
cp .env.example .env   # add your API keys

# Runtime scanning
airs runtime scan --profile "my-profile" "Is this prompt safe?"
airs runtime bulk-scan --profile "my-profile" --input prompts.csv --output results.csv

# Guardrail generation (interactive)
airs runtime topics generate

# Red team scanning
airs redteam scan --target "my-target" --type STATIC
airs redteam report <job-id>

# Model security
airs model-security scans create --group <group-id>
```

## Commands

| Command | Description |
|---------|-------------|
| `runtime scan` | Single prompt scanning against AIRS profiles |
| `runtime bulk-scan` | Batch prompt scanning with CSV output |
| `runtime topics` | Custom topic CRUD + guardrail generation (`generate`, `resume`, `report`, `runs`) |
| `runtime profiles` | Security profile CRUD + multi-topic `audit` |
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
**Required for guardrail generation:** one LLM provider key + scanning + management credentials

## License

MIT
