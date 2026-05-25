# redteam targets

### redteam targets list

List configured red team targets

```text
airs redteam targets list [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--output <format>` | No | `pretty` | Output format: pretty, table, csv, json, yaml |

#### Examples

*List all targets*

```bash
airs redteam targets list
```

```text
Targets:

89e2374c-7bac-4c5c-a291-9392ae919e14
  litellm.cdot.io - no guardrails - REST APIv2  active  type: APPLICATION
f2953fa2-943c-47aa-814d-0f421f6e071b
  AWS Bedrock - Claude 4.6  active  type: MODEL
b9e2861d-73ac-48b5-a56f-f43039cfc4a1
  postman  inactive  type: AGENT
```

---

### redteam targets get

Get target details

```text
airs redteam targets get [options] <uuid>
```

#### Arguments

- `uuid` (required) —

#### Examples

*Show full target detail (connection, background, metadata)*

```bash
airs redteam targets get 00000000-0000-0000-0000-000000000001
```

```text
Prisma AIRS — AI Red Team
Adversarial scan operations


Target Detail:

  UUID:   00000000-0000-0000-0000-000000000001
  Name:   example-target
  Status: active
  Type:   APPLICATION

  Connection:
    api_endpoint: https://api.example.com/v1/chat/completions
    request_headers: [object Object]
    request_json: [object Object]
    response_json: [object Object]
    response_key: content
    target_connection_config: null
    curl: curl \
  "https://api.example.com/v1/chat/completions" \
  -H "Content-Type: application/json" \
  -H "apikey: <api-key>" \
  --data '{"model":"<model-id>","messages":[{"role":"user","content":"{INPUT}"}]}'
    multi_turn_config: null

  Background:
    industry: Generic
    use_case: Chatbot
    competitors: Microsoft Bot Framework,Rasa,Dialogflow,Wit.ai,IBM Watson Assistant

  Metadata:
    multi_turn: false
    rate_limit: 50
    rate_limit_enabled: true
    rate_limit_error_code: 429
    content_filter_enabled: true
    content_filter_error_code: 403
    probe_message: I like turtles
    request_timeout: 110
```

---

### redteam targets create

Create a new red team target

```text
airs redteam targets create [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--config <path>` | Yes | — | JSON file with target configuration |
| `--validate` | No | — | Validate target connection before saving |

#### Examples

!!! warning "Example needed"
    No curated input/output example for this command yet.

---

### redteam targets update

Update a red team target

```text
airs redteam targets update [options] <uuid>
```

#### Arguments

- `uuid` (required) —

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--config <path>` | Yes | — | JSON file with target updates |
| `--validate` | No | — | Validate target connection before saving |

#### Examples

!!! warning "Example needed"
    No curated input/output example for this command yet.

---

### redteam targets delete

Delete a red team target

```text
airs redteam targets delete [options] <uuid>
```

#### Arguments

- `uuid` (required) —

#### Examples

!!! warning "Example needed"
    No curated input/output example for this command yet.

---

### redteam targets probe

Test target connection without saving

```text
airs redteam targets probe [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--config <path>` | Yes | — | JSON file with connection params |

#### Examples

!!! warning "Example needed"
    No curated input/output example for this command yet.

---

### redteam targets profile

View target profile

```text
airs redteam targets profile [options] <uuid>
```

#### Arguments

- `uuid` (required) —

#### Examples

*Show AI-generated target profile (truncated — full output is ~230 lines of nested JSON)*

```bash
airs redteam targets profile 00000000-0000-0000-0000-000000000001
```

```text
Prisma AIRS — AI Red Team
Adversarial scan operations

Target Profile:
  {
  "target_id": "00000000-0000-0000-0000-000000000001",
  "target_version": 1778577706636129,
  "status": "ACTIVE",
  "profiling_status": "COMPLETED",
  "target_background": {
    "industry": "Generic",
    "use_case": "Chatbot",
    "competitors": ["Rasa", "Dialogflow", "Wit.ai", "..."]
  },
  "additional_context": {
    "base_model": "transformer",
    "core_architecture": "LLM with tool calling",
    "system_prompt": null,
    "languages_supported": ["English", "Spanish", "..."],
    "banned_keywords": ["..."],
    "tools_accessible": ["web_search", "code_execution", "..."]
  },
  "ai_generated_fields": ["competitors", "base_model", "core_architecture", "..."],
  "other_details": {
    "items": {
      "code_execution": { "has_code_execution": true, "languages": ["Python"] },
      "internet_access": { "has_internet_access": true },
      "system_purpose": { "primary_purpose": "...", "use_cases": ["..."] },
      "user_audience": { "audience_type": "mixed", "features": ["..."] },
      "tool_properties": { "discoveries": ["..."] },
      "architecture": "...",
      "available_tools": "...",
      "...": "..."
    }
  }
}
```

---

### redteam targets update-profile

Update target profile

```text
airs redteam targets update-profile [options] <uuid>
```

#### Arguments

- `uuid` (required) —

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--config <path>` | Yes | — | JSON file with profile updates |

#### Examples

!!! warning "Example needed"
    No curated input/output example for this command yet.

---

### redteam targets validate-auth

Validate target auth credentials

```text
airs redteam targets validate-auth [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--auth-type <type>` | Yes | — | Auth type: HEADERS, BASIC_AUTH, OAUTH2 |
| `--config <path>` | Yes | — | JSON file with auth_config |
| `--target-id <uuid>` | No | — | Existing target UUID |

#### Examples

!!! warning "Example needed"
    No curated input/output example for this command yet.

---

### redteam targets metadata

Get target field metadata

```text
airs redteam targets metadata [options]
```

#### Examples

*Get scan-target field metadata (base models, breadth/depth ranges, NAT IPs, agent attack goal limit)*

```bash
airs redteam targets metadata
```

```text
Prisma AIRS — AI Red Team
Adversarial scan operations

{
  "scan_target": {
    "base_models": [
      "GPT3.5 turbo",
      "GPT 4",
      "Mistral 7b",
      "Mistral 13b",
      "Mixtral",
      "Claude 2.1",
      "Claude 3 sonnet",
      "Claude 3 Opus",
      "Claude 3 Haiku",
      "Gemini",
      "Llama",
      "Llama 2",
      "Llama 3",
      "Phi",
      "Falcon",
      "Bloom",
      "PaLM",
      "Others"
    ],
    "breadth": { "min": 6, "max": 12, "default": 6 },
    "depth": { "min": 3, "max": 16, "default": 10 },
    "attack_tokens": {
      "values": [128, 256, 512, 1024, 2048, 4096],
      "default": 2048
    },
    "context_size": { "min": 1, "max": 16, "default": 10 }
  },
  "nat_ips": [
    "203.0.113.10",
    "203.0.113.11",
    "203.0.113.12"
  ],
  "agent_attack_goal_limit": 10
}
```

---

### redteam targets init

Scaffold a target config JSON from a provider template

```text
airs redteam targets init [options] <provider>
```

#### Arguments

- `provider` (required) —

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--output <file>` | No | — | Output file path |

#### Examples

*Scaffold a target config JSON from a provider template (write to default filename)*

```bash
airs redteam targets init OPENAI
```

```text
Prisma AIRS — AI Red Team
Adversarial scan operations


Target config scaffolded:

  File: /path/to/cwd/openai-target.json
  Provider: OPENAI

Next steps: Edit the file to fill in name and credentials, then run:
  airs redteam targets create --config openai-target.json --validate
```

*Scaffold to a custom path*

```bash
airs redteam targets init BEDROCK --output configs/bedrock.json
```

---

### redteam targets templates

Get provider-specific target templates

```text
airs redteam targets templates [options]
```

#### Examples

*List provider-specific target templates (truncated — full output covers OPENAI, HUGGING_FACE, DATABRICKS, BEDROCK, REST, STREAMING, WEBSOCKET, MS_COPILOT_STUDIO)*

```bash
airs redteam targets templates
```

```text
Prisma AIRS — AI Red Team
Adversarial scan operations


Target Templates:

OPENAI
  {
    "id": "OPENAI",
    "name": "OpenAI",
    "is_custom": false,
    "url": "https://api.openai.com/v1/responses",
    "request_json": {
      "model": "gpt-4.1-mini",
      "input": [
        {
          "role": "user",
          "content": [
            { "type": "input_text", "text": "{INPUT}" }
          ]
        }
      ],
      "stream": false,
      "temperature": 0.7
    },
    "response_json": {
      "id": "resp_xxxxx",
      "object": "response",
      "model": "gpt-4.1-mini",
      "output": [
        {
          "type": "message",
          "role": "assistant",
          "content": [
            { "type": "output_text", "text": "{RESPONSE}" }
          ]
        }
      ]
    }
  }

HUGGING_FACE
  { ... }

DATABRICKS
  { ... }

BEDROCK
  { ... }

REST
  { ... }

STREAMING
  { ... }

WEBSOCKET
  { ... }

MS_COPILOT_STUDIO
  { ... }
```

---

### redteam targets backup

Backup red team targets to local JSON/YAML files

```text
airs redteam targets backup [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--output-dir <path>` | No | — | Output directory |
| `--format <format>` | No | `json` | Output format: json or yaml |
| `--name <targetName>` | No | — | Backup a single target by name |

#### Examples

*Back up a single target by name to a local directory*

```bash
airs redteam targets backup --output-dir ./backups --name "example-target"
```

```text
Prisma AIRS — Backup & Restore


Backed up 1 target(s) to ./backups:

  ✓ example-target → example-target.json
```

*Back up all targets in YAML format*

```bash
airs redteam targets backup --output-dir ./backups --format yaml
```

---

### redteam targets restore

Restore red team targets from local JSON/YAML backup files

```text
airs redteam targets restore [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--input-dir <path>` | No | — | Directory containing backup files |
| `--file <path>` | No | — | Single backup file to restore |
| `--overwrite` | No | — | Update existing targets with same name (default: skip) |
| `--validate` | No | — | Validate target connection before saving |

#### Examples

*Restore from a directory of backup files (skips targets that already exist by name)*

```bash
airs redteam targets restore --input-dir ./backups
```

```text
Prisma AIRS — Backup & Restore


Restore results:

  ○ example-target — skipped

Total: 1 skipped
```

*Restore a single file and overwrite an existing target*

```bash
airs redteam targets restore --file ./backups/example-target.json --overwrite
```
