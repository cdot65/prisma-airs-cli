# Prisma AIRS CLI

[![CI](https://github.com/cdot65/prisma-airs-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/cdot65/prisma-airs-cli/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Node 20+](https://img.shields.io/badge/node-%3E%3D20-brightgreen.svg)](https://nodejs.org/)

**Full operational coverage over Palo Alto Prisma AIRS AI security — guardrail refinement, runtime scanning, AI red teaming, model security, and profile audits.**

> **[Read the full documentation](https://cdot65.github.io/prisma-airs-cli/)** — installation, configuration, architecture, CLI reference, and examples.

## Install

```bash
npm install -g @cdot65/prisma-airs-cli
```

Requires **Node.js >= 20**.

## Quick Start

```bash
cp .env.example .env   # add your API keys
airs runtime topics generate   # interactive guardrail generation
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
| `runtime scan-logs` | Scan log querying |
| `redteam` | Adversarial scanning — targets, prompt sets, scans, reports |
| `model-security` | ML model supply chain security — groups, rules, scans, labels |

## License

MIT
