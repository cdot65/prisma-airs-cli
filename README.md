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

## DLP Management

Manage Data Loss Prevention resources across filtering profiles, patterns, profiles, and dictionaries. Each subclient supports different operation patterns:

- **Filtering profiles** — read-only + full replace
- **Patterns** — full CRUD with soft-delete via patch
- **Profiles** — full CRUD, use patch to soft-delete via `profile_status: "deleted"`
- **Dictionaries** — create, replace, and delete with multipart upload support

### Filtering Profiles

Read-only access with full-replace support:

```bash
# List all filtering profiles
airs runtime dlp filtering-profiles list --output json

# Get a specific filtering profile
airs runtime dlp filtering-profiles get <id>

# Replace a filtering profile
airs runtime dlp filtering-profiles replace <id> --body-file profile.json
```

### Patterns

Full CRUD with soft-delete:

```bash
# List patterns
airs runtime dlp patterns list

# Get a pattern
airs runtime dlp patterns get <id>

# Create a new pattern
airs runtime dlp patterns create --body-file pattern.json

# Update a pattern
airs runtime dlp patterns patch <id> --set name="Updated Name"

# Soft-delete a pattern
airs runtime dlp patterns patch <id> --set is_archived=true

# Hard delete a pattern
airs runtime dlp patterns delete <id>
```

### Profiles

Full CRUD with soft-delete via `profile_status`:

```bash
# List profiles
airs runtime dlp profiles list

# Get a profile
airs runtime dlp profiles get <id>

# Create a profile
airs runtime dlp profiles create --body-file profile.json

# Update a profile
airs runtime dlp profiles patch <id> --body-file - <<EOF
{ "name": "my-profile", "profile_type": "advanced", "description": "Updated description" }
EOF

# Soft-delete (set profile_status to "deleted")
airs runtime dlp profiles patch <id> --body-file - <<EOF
{ "profile_status": "deleted" }
EOF

# Hard delete a profile
airs runtime dlp profiles delete <id>
```

### Dictionaries

Multipart upload support for dictionary management:

```bash
# List dictionaries
airs runtime dlp dictionaries list

# Get a dictionary
airs runtime dlp dictionaries get <id>

# Create a dictionary with file upload
airs runtime dlp dictionaries create --name "Allowlist" --category Confidential \
  --region us --file keywords.txt

# Replace a dictionary (multipart upload)
airs runtime dlp dictionaries replace <id> --file keywords.txt --name "Allowlist" \
  --category Confidential --region us

# Delete a dictionary
airs runtime dlp dictionaries delete <id>
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
| `runtime dlp filtering-profiles` | DLP filtering profile listing and full-replace |
| `runtime dlp filtering-profiles list` | List all filtering profiles |
| `runtime dlp filtering-profiles get` | Get a specific filtering profile |
| `runtime dlp filtering-profiles replace` | Replace a filtering profile |
| `runtime dlp patterns` | DLP pattern CRUD with soft-delete |
| `runtime dlp patterns list` | List all patterns |
| `runtime dlp patterns get` | Get a specific pattern |
| `runtime dlp patterns create` | Create a new pattern |
| `runtime dlp patterns patch` | Update a pattern or soft-delete via `is_archived` |
| `runtime dlp patterns delete` | Hard delete a pattern |
| `runtime dlp profiles` | DLP profile CRUD with soft-delete via `profile_status` |
| `runtime dlp profiles list` | List all profiles |
| `runtime dlp profiles get` | Get a specific profile |
| `runtime dlp profiles create` | Create a new profile |
| `runtime dlp profiles patch` | Update a profile or soft-delete via `profile_status: "deleted"` |
| `runtime dlp profiles delete` | Hard delete a profile |
| `runtime dlp dictionaries` | DLP dictionary management with multipart upload |
| `runtime dlp dictionaries list` | List all dictionaries |
| `runtime dlp dictionaries get` | Get a specific dictionary |
| `runtime dlp dictionaries create` | Create a dictionary with file upload |
| `runtime dlp dictionaries replace` | Replace a dictionary with multipart upload |
| `runtime dlp dictionaries delete` | Delete a dictionary |
| `redteam scan` | Adversarial scanning (STATIC, DYNAMIC, CUSTOM) |
| `redteam targets` | Red team target CRUD |
| `redteam prompt-sets` | Custom prompt set management |
| `model-security groups` | Security group CRUD |
| `model-security rules` | Security rule management |
| `model-security scans` | Model security scanning |
| `backup targets` | Export red team targets to local JSON/YAML files |
| `restore targets` | Import red team targets from backup files |

## Configuration

Credentials are configured via environment variables or `~/.prisma-airs/config.json`. See [`.env.example`](.env.example) for the full list.

| Variable | Purpose | Required |
|----------|---------|----------|
| `PANW_AI_SEC_API_KEY` | Scanning API authentication | Yes (for scanning) |
| `PANW_MGMT_CLIENT_ID` | Management API OAuth client ID | Yes (for management) |
| `PANW_MGMT_CLIENT_SECRET` | Management API OAuth client secret | Yes (for management) |
| `PANW_MGMT_TSG_ID` | Management API tenant security group ID | Yes (for management) |
| `PANW_DLP_ENDPOINT` | DLP API base URL override | No (defaults to `api.dlp.paloaltonetworks.com`) |
| `LLM_PROVIDER` | LLM provider for profile audits | Yes (for audit) |
| `ANTHROPIC_API_KEY` | Claude API key | Yes (if using Claude) |
| `GOOGLE_API_KEY` | Google Gemini API key | Yes (if using Gemini) |

**Required for scanning:** `PANW_AI_SEC_API_KEY`
**Required for management:** `PANW_MGMT_CLIENT_ID`, `PANW_MGMT_CLIENT_SECRET`, `PANW_MGMT_TSG_ID`
**Required for profile audits:** one LLM provider key + scanning + management credentials

## Live Smoke Testing

The unit suite uses fully mocked services — zero tests in this repo hit a real Prisma AIRS tenant. After every CLI release, after upgrading the SDK, or after an AIRS backend rollout, run the [live smoke test reference](https://cdot65.github.io/prisma-airs-cli/development/smoke-tests/) ([source](docs/development/smoke-tests.md)). Sixteen read-only commands across runtime security, red team, and model security that exercise the SDK's runtime Zod validation against your tenant.

## License

MIT
