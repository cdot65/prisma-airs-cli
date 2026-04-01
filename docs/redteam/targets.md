---
title: Managing Targets
---

# Managing Targets

Red team targets represent the AI applications you want to test. Full CRUD operations are available via `airs redteam targets`.

## Prerequisites

- Prisma AIRS CLI installed and configured ([Installation](../getting-started/installation.md))
- AIRS management credentials set (`PANW_MGMT_CLIENT_ID`, `PANW_MGMT_CLIENT_SECRET`, `PANW_MGMT_TSG_ID`)

## List Targets

```bash
airs redteam targets list
```

Each target shows its UUID, name, status (`active`/`inactive`), and type (`APPLICATION`, `MODEL`, `AGENT`).

## Initialize a Target from Template

Instead of writing a JSON config from scratch, scaffold one from a provider template:

```bash
airs redteam targets init <provider>
```

Available providers: `OPENAI`, `HUGGING_FACE`, `DATABRICKS`, `BEDROCK`, `REST`, `STREAMING`, `WEBSOCKET`.

This fetches the provider's template from AIRS and writes a ready-to-edit JSON file:

```bash
# Scaffold an OpenAI target config
airs redteam targets init openai

# Custom output path
airs redteam targets init bedrock --output my-bedrock-target.json
```

The scaffolded file contains the provider's connection template with the correct request/response structure. Edit it to fill in your `name` and credentials, then create the target:

```bash
# Edit the scaffolded file
vim openai-target.json

# Create the target (with connection validation)
airs redteam targets create --config openai-target.json --validate
```

!!! tip
    The `init` command is the fastest way to get started — it gives you the correct JSON structure for your provider so you only need to fill in credentials.

## Create a Target

Targets are created from a JSON configuration file with `name`, `target_type`, and `connection_params`:

**Example `target.json`:**

```json
{
  "name": "docs-example-target",
  "target_type": "APPLICATION",
  "connection_params": {
    "api_endpoint": "https://litellm.cdot.io/v1/chat/completions",
    "request_headers": {
      "Content-Type": "application/json",
      "Authorization": "Bearer sk-your-api-key"
    },
    "request_json": {
      "model": "mistral-7b",
      "messages": [{"role": "user", "content": "{INPUT}"}],
      "max_tokens": 256
    },
    "response_json": {
      "choices": [{"message": {"content": "{RESPONSE}"}}]
    },
    "response_key": "content"
  }
}
```

!!! warning "Required placeholders"
    - `{INPUT}` in `request_json` — where the red team injects prompts
    - `{RESPONSE}` in `response_json` — where the target's response is extracted

```bash
airs redteam targets create --config target.json
```

The target starts as `inactive` until validated. Use `--validate` to test the connection on creation:

```bash
airs redteam targets create --config target.json --validate
```

## Get Target Details

```bash
airs redteam targets get <uuid>
```

Shows full configuration including connection parameters and a ready-to-use `curl` command (credentials masked).

## View Target Profile

Targets that have been profiled contain detailed context about the AI application:

```bash
airs redteam targets profile <uuid>
```

The profile includes target background (industry, use case), additional context (system prompt, base model, tools), and profiling status.

!!! info "Profiling"
    Profiling is initiated from the AIRS console. Prisma AIRS CLI can read the profile but profiling itself runs server-side.

## Update a Target

```bash
airs redteam targets update <uuid> --config updates.json
```

The AIRS API requires `target_type` and full `connection_params` on every update. Add `--validate` to re-test connectivity:

```bash
airs redteam targets update <uuid> --config updates.json --validate
```

## Probe a Connection

Test a target connection without saving it:

```bash
airs redteam targets probe --config connection.json
```

## Delete a Target

```bash
airs redteam targets delete <uuid>
```

!!! warning
    Deleting a target is permanent. Existing scan results are retained, but no new scans can be launched against a deleted target.

## Validate Target Auth

Test authentication credentials against a target without creating or modifying it:

```bash
airs redteam targets validate-auth --config auth.json
```

**Example `auth.json`:**

```json
{
  "auth_type": "HEADERS",
  "auth_config": {
    "headers": {
      "Authorization": "Bearer sk-your-api-key"
    }
  }
}
```

Optionally pass `--target <uuid>` to validate against an existing target's endpoint.

Supported auth types: `HEADERS`, `BASIC_AUTH`, `OAUTH2`.

## Get Target Metadata

Retrieve field metadata describing valid target configuration options:

```bash
airs redteam targets metadata
```

Returns JSON describing available target types, connection parameter fields, and their validation rules.

## Get Target Templates

Retrieve provider-specific target configuration templates:

```bash
airs redteam targets templates
```

Returns pre-built JSON templates for common AI providers (OpenAI, Azure OpenAI, AWS Bedrock, etc.) that can be used as starting points for `targets create --config`.

---

## JSON Config Reference

| Field | Required | Description |
|-------|:--------:|-------------|
| `name` | Yes | Human-readable target name |
| `target_type` | Yes | `APPLICATION`, `MODEL`, or `AGENT` |
| `connection_params.api_endpoint` | Yes | Full URL of your AI endpoint |
| `connection_params.request_headers` | Yes | HTTP headers (auth, content-type) |
| `connection_params.request_json` | Yes | Request body template with `{INPUT}` placeholder |
| `connection_params.response_json` | Yes | Response body template with `{RESPONSE}` placeholder |
| `connection_params.response_key` | Yes | Key to extract the response text |
| `background.industry` | No | Industry vertical |
| `background.use_case` | No | Application use case |
| `metadata.multi_turn` | No | Whether target supports multi-turn conversations |
| `metadata.rate_limit` | No | Requests per second cap |
