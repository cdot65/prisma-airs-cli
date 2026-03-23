# AI Agent Instructions — Prisma AIRS CLI

This document instructs AI agents (Claude Code, Gemini CLI, etc.) on how to use the `airs` CLI to interact with Palo Alto Prisma AIRS. It is a machine-readable reference — not user documentation. For human docs see `docs/` or <https://cdot65.github.io/prisma-airs-cli/>.

---

## Quick Context

`airs` is a CLI for Palo Alto Prisma AIRS AI security platform. It covers:

1. **Runtime Security** — scan prompts against AIRS security profiles, manage profiles/topics/API keys
2. **Guardrail Generation** — LLM-driven iterative refinement of custom topic definitions
3. **AI Red Teaming** — adversarial scans against targets with static/dynamic/custom attacks
4. **Model Security** — ML model supply chain scanning, security groups, rules, violations
5. **Profile Audits** — multi-topic evaluation with conflict detection

The binary is `airs`. Three top-level command groups: `runtime`, `redteam`, `model-security`. Global flag `--debug` logs all AIRS/SCM API requests and responses to `~/.prisma-airs/debug-api-<timestamp>.jsonl`.

---

## Authentication

Different commands require different credentials. Set these as environment variables or in `~/.prisma-airs/config.json`.

### Credential Sets

| Credential Set | Environment Variables | Used By |
|---|---|---|
| **Scanner API** | `PANW_AI_SEC_API_KEY` | `runtime scan`, `runtime bulk-scan`, `runtime profiles audit`, guardrail generation |
| **Management API** (OAuth2) | `PANW_MGMT_CLIENT_ID`, `PANW_MGMT_CLIENT_SECRET`, `PANW_MGMT_TSG_ID` | All CRUD commands (profiles, topics, api-keys, customer-apps), all redteam commands, all model-security commands |
| **LLM Provider** | Depends on provider (see below) | `runtime topics generate`, `runtime topics resume`, `runtime profiles audit` |

### LLM Providers

| Provider Value | Required Env Vars | Default Model |
|---|---|---|
| `claude-api` | `ANTHROPIC_API_KEY` | `claude-opus-4-6` |
| `claude-vertex` | `GOOGLE_CLOUD_PROJECT` | `claude-opus-4-6` |
| `claude-bedrock` | `AWS_REGION`, `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY` | `anthropic.claude-opus-4-6-v1` |
| `gemini-api` | `GOOGLE_API_KEY` | `gemini-2.5-pro` |
| `gemini-vertex` | `GOOGLE_CLOUD_PROJECT` | `gemini-2.5-pro` |
| `gemini-bedrock` | `AWS_REGION`, `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY` | `gemini-2.5-pro` |

### Optional Management Endpoints

| Variable | Purpose |
|---|---|
| `PANW_MGMT_ENDPOINT` | Override management API base URL |
| `PANW_MGMT_TOKEN_ENDPOINT` | Override OAuth2 token URL |

### Verifying Credentials

```bash
# Scanner API — should return scan result
airs runtime scan --profile <profile-name> "test prompt"

# Management API — should return list of profiles
airs runtime profiles list

# Red Team — should return targets
airs redteam targets list

# Model Security — should return groups
airs model-security groups list
```

---

## Output Formats

All list commands accept `--output <format>`:

| Format | Description | Best For |
|---|---|---|
| `pretty` | Colored terminal output (default) | Human reading |
| `table` | ASCII table with box-drawing chars | Human reading, aligned columns |
| `csv` | RFC 4180 CSV with header row | Piping to files, spreadsheets |
| `json` | Pretty-printed JSON array | Programmatic parsing by agents |
| `yaml` | YAML with `---` separators | Config files, readability |

**For AI agents: always use `--output json` when you need to parse results programmatically.**

```bash
# Get parseable JSON output
airs runtime profiles list --output json
airs redteam targets list --output json
airs model-security groups list --output json
```

---

## Command Reference

### Runtime — Scanning

#### Scan a single prompt

```bash
airs runtime scan --profile <profile-name> "<prompt>"
airs runtime scan --profile <profile-name> --response "<response>" "<prompt>"
```

**Required:** `--profile`, `<prompt>` argument
**Optional:** `--response` (scan prompt+response pair)
**Auth:** Scanner API key

**Output fields:** Action (block/allow), Category, Triggered (yes/no), Scan ID, Report ID, Detections list

#### Bulk scan from file

```bash
airs runtime bulk-scan --profile <profile-name> --input <file> [--output <csv>] [--session-id <id>]
```

**Required:** `--profile`, `--input`
**Input formats:** `.csv` (extracts `prompt` column by header), `.txt` (one prompt per line)
**Output:** CSV file with results (default: `<profile>-bulk-scan.csv`)
**Auth:** Scanner API key

**Behavior:** Batches prompts in groups of 5, submits async, polls with exponential backoff on rate limit. Saves scan IDs to `~/.prisma-airs/bulk-scans/` for crash recovery.

#### Resume polling

```bash
airs runtime resume-poll <stateFile> [--output <csv>]
```

Resumes polling from saved bulk scan state file after a crash or rate limit failure.

---

### Runtime — Configuration Management

All CRUD commands require Management API credentials.

#### Security Profiles

```bash
airs runtime profiles list [--limit <n>] [--offset <n>] [--output <format>]
airs runtime profiles get <nameOrId> [--output <pretty|json|yaml>]
airs runtime profiles create --name <name> [protection flags...]
airs runtime profiles update <profileId> [protection flags...]
airs runtime profiles delete <profileId> [--force --updated-by <email>]
```

**`create`** requires `--name`. All protection flags are optional — omitted sections get AIRS defaults.

**`update`** uses read-modify-write: fetches the current profile, merges only the flags you specify, PUTs the full payload. Existing policy sections (including topic-guardrails) are preserved. Only specify what you want to change.

**Profile protection flags** (available on both `create` and `update`):

| Flag | Description |
|------|-------------|
| `--prompt-injection <action>` | Prompt injection action (`block`/`allow`/`alert`) |
| `--toxic-content <action>` | Toxic content action (e.g. `"high:block, moderate:block"`) |
| `--contextual-grounding <action>` | Contextual grounding action |
| `--malicious-code <action>` | Malicious code protection action |
| `--url-action <action>` | URL detected action |
| `--allow-url-categories <list>` | Comma-separated URL categories to allow |
| `--block-url-categories <list>` | Comma-separated URL categories to block |
| `--alert-url-categories <list>` | Comma-separated URL categories to alert |
| `--agent-security <action>` | Agent security action |
| `--dlp-action <action>` | Data leak detection action |
| `--dlp-profiles <list>` | Comma-separated DLP profile names |
| `--mask-data-inline` | Mask detected data inline (boolean) |
| `--db-security-create <action>` | Database create action |
| `--db-security-read <action>` | Database read action |
| `--db-security-update <action>` | Database update action |
| `--db-security-delete <action>` | Database delete action |
| `--inline-timeout-action <action>` | Inline timeout action (`block`/`allow`) |
| `--max-inline-latency <n>` | Max inline latency in seconds |
| `--mask-data-in-storage` | Mask data in storage (boolean) |
| `--no-active` | Create/set profile as inactive |

**Examples:**

```bash
# Create with multiple protections
airs runtime profiles create --name "Prod Firewall" \
  --prompt-injection block \
  --toxic-content "high:block, moderate:alert" \
  --malicious-code block \
  --agent-security block

# Update only one setting (everything else preserved)
airs runtime profiles update <profileId> --toxic-content "high:alert"
```

#### Custom Topics

```bash
airs runtime topics list [--limit <n>] [--offset <n>] [--output <format>]
airs runtime topics create --config <json-file>
airs runtime topics update <topicId> --config <json-file>
airs runtime topics delete <topicId> [--force --updated-by <email>]
```

#### API Keys

```bash
airs runtime api-keys list [--limit <n>] [--output <format>]
airs runtime api-keys create --config <json-file>
airs runtime api-keys regenerate <apiKeyId> --interval <n> --unit <unit> [--updated-by <email>]
airs runtime api-keys delete <apiKeyName> --updated-by <email>
```

`--unit` values: `hours`, `days`, `months`

#### Customer Apps

```bash
airs runtime customer-apps list [--limit <n>] [--output <format>]
airs runtime customer-apps get <appName>
airs runtime customer-apps update <appId> --config <json-file>
airs runtime customer-apps delete <appName> --updated-by <email>
```

#### Deployment Profiles (read-only)

```bash
airs runtime deployment-profiles list [--unactivated] [--output <format>]
```

#### DLP Profiles (read-only)

```bash
airs runtime dlp-profiles list [--output <format>]
```

#### Scan Logs

```bash
airs runtime scan-logs query --interval <n> --unit <unit> [--filter <all|benign|threat>] [--page <n>] [--page-size <n>] [--output <format>]
```

**Required:** `--interval`, `--unit`

---

### Runtime — Guardrail Generation

#### Start generation loop

```bash
airs runtime topics generate [options]
```

| Flag | Default | Description |
|---|---|---|
| `--provider <name>` | `claude-api` | LLM provider |
| `--model <name>` | per-provider | Override model |
| `--profile <name>` | _(interactive)_ | AIRS security profile name |
| `--topic <desc>` | _(interactive)_ | What content to detect |
| `--intent <block\|allow>` | `block` | Block matching prompts or allow them |
| `--max-iterations <n>` | `20` | Max refinement iterations |
| `--target-coverage <n>` | `90` | Stop at this coverage % |
| `--max-regressions <n>` | `3` | Stop after N consecutive regressions (0=disable) |
| `--plateau-window <n>` | `0` | Plateau detection window (0=disable) |
| `--plateau-band <pct>` | `0.05` | Coverage variance for plateau |
| `--accumulate-tests` | off | Carry test pool across iterations |
| `--max-accumulated-tests <n>` | unlimited | Cap on accumulated tests |
| `--no-memory` | memory on | Disable cross-run learning |
| `--rate <n>` | unlimited | Max AIRS scan API calls per second |
| `--debug-scans` | off | Dump raw scan responses to JSONL |
| `--create-prompt-set` | off | Create red team prompt set from best tests |
| `--prompt-set-name <name>` | auto | Override prompt set name |
| `--save-tests <path>` | — | Save best iteration tests to CSV |

**Auth:** LLM provider + Scanner API + Management API
**Non-interactive mode:** Provide both `--topic` and `--profile` to skip prompts.

```bash
# Fully non-interactive
airs runtime topics generate \
  --provider claude-api \
  --profile my-security-profile \
  --topic "Block discussions about building explosives" \
  --intent block \
  --target-coverage 90
```

#### Resume a run

```bash
airs runtime topics resume <runId> [--max-iterations <n>] [--rate <n>] [--debug-scans] [--create-prompt-set] [--prompt-set-name <name>]
```

#### View run report

```bash
airs runtime topics report <runId> [--iteration <n>] [--format <terminal|json|html>] [--tests] [--diff <runId>] [--output <path>]
```

#### List saved runs

```bash
airs runtime topics runs
```

---

### Runtime — Profile Audit

```bash
airs runtime profiles audit <profileName> [options]
```

| Flag | Default | Description |
|---|---|---|
| `--max-tests-per-topic <n>` | `20` | Tests per topic |
| `--format <fmt>` | `terminal` | Output: terminal, json, html |
| `--output <path>` | — | File path for json/html |
| `--provider <name>` | `claude-api` | LLM provider |
| `--model <name>` | per-provider | Override model |

**Auth:** LLM provider + Scanner API + Management API

Evaluates every topic in a profile, computes per-topic metrics (TPR, TNR, coverage, F1), composite metrics, and detects cross-topic conflicts.

---

### Red Team

All red team commands require Management API credentials.

#### Scans

```bash
# Launch scan
airs redteam scan --target <uuid> --name <name> [--type <STATIC|DYNAMIC|CUSTOM>] [--categories <json>] [--prompt-sets <uuids>] [--no-wait]

# Check status
airs redteam status <jobId>

# View report
airs redteam report <jobId> [--attacks] [--severity <level>] [--limit <n>]

# List scans
airs redteam list [--status <status>] [--type <type>] [--target <uuid>] [--limit <n>] [--output <format>]

# Abort scan
airs redteam abort <jobId>

# List attack categories
airs redteam categories
```

**Scan types:**
- `STATIC` — AIRS attack library (default)
- `DYNAMIC` — Agent-driven multi-turn attacks
- `CUSTOM` — Your prompt sets (requires `--prompt-sets <uuid1>,<uuid2>`)

**CRITICAL:** `--prompt-sets` takes comma-separated UUID strings, NOT JSON objects.

#### Targets

```bash
airs redteam targets list [--output <format>]
airs redteam targets get <uuid>
airs redteam targets create --config <json-file> [--validate]
airs redteam targets update <uuid> --config <json-file> [--validate]
airs redteam targets delete <uuid>
airs redteam targets probe --config <json-file>
airs redteam targets profile <uuid>
airs redteam targets update-profile <uuid> --config <json-file>
```

`--validate` tests the connection before saving.

**Target config JSON example:**
```json
{
  "name": "My Chatbot",
  "target_type": "REST",
  "connection_params": {
    "api_endpoint": "https://api.example.com/chat",
    "request_headers": { "Authorization": "Bearer token" },
    "request_json": { "message": "{prompt}" },
    "response_key": "response"
  },
  "background": {
    "industry": "finance",
    "use_case": "customer support"
  },
  "metadata": {
    "multi_turn": false,
    "rate_limit": 10
  }
}
```

#### Prompt Sets

```bash
airs redteam prompt-sets list [--output <format>]
airs redteam prompt-sets get <uuid>
airs redteam prompt-sets create --name <name> [--description <desc>]
airs redteam prompt-sets update <uuid> [--name <name>] [--description <desc>]
airs redteam prompt-sets archive <uuid> [--unarchive]
airs redteam prompt-sets download <uuid> [--output <path>]
airs redteam prompt-sets upload <uuid> <csv-file>
```

#### Individual Prompts

```bash
airs redteam prompts list <setUuid> [--limit <n>]
airs redteam prompts get <setUuid> <promptUuid>
airs redteam prompts add <setUuid> --prompt <text> [--goal <text>]
airs redteam prompts update <setUuid> <promptUuid> [--prompt <text>] [--goal <text>]
airs redteam prompts delete <setUuid> <promptUuid>
```

#### Properties

```bash
airs redteam properties list [--output <format>]
airs redteam properties create --name <name>
airs redteam properties values <name>
airs redteam properties add-value --name <name> --value <value>
```

---

### Model Security

All model security commands require Management API credentials.

#### Security Groups

```bash
airs model-security groups list [--source-types <types>] [--search <query>] [--sort-field <field>] [--sort-dir <asc|desc>] [--enabled-rules <uuids>] [--limit <n>] [--output <format>]
airs model-security groups get <uuid>
airs model-security groups create --config <json-file>
airs model-security groups update <uuid> [--name <name>] [--description <desc>]
airs model-security groups delete <uuid>
```

**Group config JSON:**
```json
{
  "name": "My Security Group",
  "source_type": "LOCAL",
  "description": "Scans local model files",
  "rule_configurations": {}
}
```

Source types: `LOCAL`, `S3`, `GCS`, `AZURE`, `HUGGING_FACE`

#### Rules (read-only)

```bash
airs model-security rules list [--source-type <type>] [--search <query>] [--limit <n>] [--output <format>]
airs model-security rules get <uuid>
```

#### Rule Instances

```bash
airs model-security rule-instances list <groupUuid> [--security-rule-uuid <uuid>] [--state <DISABLED|ALLOWING|BLOCKING>] [--limit <n>]
airs model-security rule-instances get <groupUuid> <instanceUuid>
airs model-security rule-instances update <groupUuid> <instanceUuid> --config <json-file>
```

**Rule instance update JSON:**
```json
{
  "state": "BLOCKING",
  "field_values": { "threshold": 0.8 }
}
```

#### Scans

```bash
airs model-security scans list [--eval-outcome <outcome>] [--source-type <type>] [--scan-origin <origin>] [--search <query>] [--limit <n>] [--output <format>]
airs model-security scans get <uuid>
airs model-security scans create --config <json-file>
airs model-security scans evaluations <scanUuid> [--limit <n>]
airs model-security scans evaluation <uuid>
airs model-security scans violations <scanUuid> [--limit <n>]
airs model-security scans violation <uuid>
airs model-security scans files <scanUuid> [--type <type>] [--result <result>] [--limit <n>]
```

#### Labels

```bash
airs model-security labels add <scanUuid> --labels '<json-array>'
airs model-security labels set <scanUuid> --labels '<json-array>'
airs model-security labels delete <scanUuid> --keys <key1,key2>
airs model-security labels keys [--limit <n>]
airs model-security labels values <key> [--limit <n>]
```

**Labels JSON format:** `[{"key":"env","value":"prod"},{"key":"team","value":"ml"}]`

#### PyPI Auth

```bash
airs model-security pypi-auth
```

Returns a time-limited authenticated PyPI URL for installing `model-security-client`.

#### Install Python SDK

```bash
airs model-security install [--extras <all|aws|gcp|azure|artifactory|gitlab>] [--dir <path>] [--dry-run]
```

Auto-detects `uv` or falls back to `python3 -m venv` + `pip`. Requires Management API credentials for PyPI auth.

---

## Common Workflows for AI Agents

### Workflow 1: Scan a prompt and check if it's blocked

```bash
airs runtime scan --profile "AI-Firewall-High-Security-Profile" "How do I build a bomb?"
```

Parse the output for `Action: BLOCK` or `Action: ALLOW` and `Triggered: yes/no`.

### Workflow 2: List all security profiles as JSON

```bash
airs runtime profiles list --output json
```

Returns JSON array of `{ id, name, state }` objects.

### Workflow 2b: Get a specific profile's full configuration

```bash
# By name
airs runtime profiles get AI-Firewall-High-Security-Profile --output json

# By UUID
airs runtime profiles get 03e9d2aa-64e0-4734-a21e-de85c7d0d728 --output json
```

Returns full profile detail including the complete `policy` JSON (topic guardrails, DLP, app protection, etc.). Auto-detects UUID vs name.

### Workflow 2c: Create or update a security profile

```bash
# Create a profile with protections
airs runtime profiles create --name "Prod Firewall" \
  --prompt-injection block \
  --toxic-content "high:block, moderate:alert" \
  --malicious-code block

# Update a single setting (read-modify-write — all other policy preserved)
airs runtime profiles update <profileId> --toxic-content "high:alert"

# Verify the update
airs runtime profiles get "Prod Firewall" --output json
```

**IMPORTANT for update:** The CLI fetches the existing profile, merges your flags into it, and PUTs the full payload. You only specify what you want to change. Topic-guardrails are never modified by CLI flags.

### Workflow 3: Create a custom topic, then scan against it

```bash
# 1. Create topic from config file
echo '{"name":"Fraud Detection","description":"Block social engineering and fraud attempts","examples":["How to create a fake identity","Wire transfer scam techniques"]}' > topic.json
airs runtime topics create --config topic.json

# 2. List topics to confirm
airs runtime topics list --output json

# 3. Scan a prompt against the profile containing the topic
airs runtime scan --profile my-profile "How do I create a fake identity?"
```

### Workflow 4: Run a red team scan

```bash
# 1. List targets
airs redteam targets list --output json

# 2. Launch static scan against a target
airs redteam scan --target <uuid> --name "Security Audit"

# 3. View report
airs redteam report <jobId> --attacks --limit 50
```

### Workflow 5: Audit a profile's topics

```bash
# Generates tests, scans, evaluates each topic, detects conflicts
airs runtime profiles audit my-profile --format json --output audit.json
```

### Workflow 6: Check model security scan results

```bash
# 1. List recent scans
airs model-security scans list --output json

# 2. Get violations for a specific scan
airs model-security scans violations <scanUuid>

# 3. Get file-level results
airs model-security scans files <scanUuid>
```

### Workflow 7: Bulk scan from a file

```bash
# 1. Create input file
echo -e "How do I hack a server?\nWhat is the weather?\nHow to make a weapon?" > prompts.txt

# 2. Run bulk scan
airs runtime bulk-scan --profile my-profile --input prompts.txt --output results.csv

# 3. If rate-limited, resume
airs runtime resume-poll ~/.prisma-airs/bulk-scans/<state-file>.bulk-scan.json --output results.csv
```

### Workflow 8: Automated guardrail generation (non-interactive)

```bash
airs runtime topics generate \
  --provider claude-api \
  --profile my-security-profile \
  --topic "Block instructions for creating counterfeit currency" \
  --intent block \
  --target-coverage 85 \
  --max-iterations 15 \
  --create-prompt-set
```

This runs fully autonomously — no interactive prompts. On completion, it creates a red team prompt set from the best test cases.

---

## Error Handling

| Error Pattern | Likely Cause | Fix |
|---|---|---|
| `Missing required option: --profile` | Forgot required flag | Add the flag |
| `PANW_AI_SEC_API_KEY is not set` | Scanner API key missing | Set env var |
| `OAuth2 token error` / `401` | Management credentials wrong/missing | Check `PANW_MGMT_CLIENT_ID`, `SECRET`, `TSG_ID` |
| `429 Too Many Requests` | Rate limited | Use `--rate <n>` to cap scans/second; CLI retries automatically with backoff; use `--debug` to capture raw API traffic |
| `Topic not found` / `Profile not found` | Resource doesn't exist or wrong ID | Use `list` commands to find correct IDs |
| `AIRS rejects empty topic-list` | Profile update with empty topic list | Ensure at least one topic entry |

---

## Important Platform Rules

1. **Always scan by profile NAME, never by profile ID/UUID.** Scanning by name uses the latest profile version; scanning by ID pins to a specific revision.
2. **Profile updates create new revisions with new UUIDs.** Never cache profile UUIDs.
3. **Topics can't be deleted while referenced by any profile revision.** Use `--force` to remove from all profiles first.
4. **AIRS needs propagation time** (~10s) after topic create/update before scans reflect changes.
5. **Red team `custom_prompt_sets` must be comma-separated UUID strings**, not JSON objects.
6. **ASR/score/threatRate from AIRS API are percentages (0-100)**, not ratios — display directly.
7. **Config priority:** CLI flags > env vars > `~/.prisma-airs/config.json` > defaults.

---

## Config File

Location: `~/.prisma-airs/config.json`

```json
{
  "llmProvider": "claude-api",
  "anthropicApiKey": "sk-...",
  "airsApiKey": "...",
  "mgmtClientId": "...",
  "mgmtClientSecret": "...",
  "mgmtTsgId": "...",
  "scanConcurrency": 5,
  "propagationDelayMs": 10000,
  "memoryEnabled": true,
  "memoryDir": "~/.prisma-airs/memory",
  "dataDir": "~/.prisma-airs/runs",
  "maxMemoryChars": 3000,
  "accumulateTests": false
}
```

All fields are optional — env vars and CLI flags take precedence.
