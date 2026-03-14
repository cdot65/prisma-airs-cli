# Managing Red Team Targets

This walkthrough demonstrates full CRUD operations for AI Red Team targets — create, inspect, update, and delete targets that represent your AI applications.

All output shown below is from real commands run against Prisma AIRS.

## Prerequisites

- Prisma AIRS CLI installed and configured ([Installation](../getting-started/installation.md))
- AIRS management credentials set (`PANW_MGMT_CLIENT_ID`, `PANW_MGMT_CLIENT_SECRET`, `PANW_MGMT_TSG_ID`)

## List Targets

View all configured red team targets:

```bash
airs redteam targets list
```

```
  Prisma AIRS — AI Red Team
  Adversarial scan operations


  Targets:

  89e2374c-7bac-4c5c-a291-9392ae919e14
    litellm.cdot.io - no guardrails - REST APIv2  active  type: APPLICATION
  4978ebcf-bc4d-4c83-90f2-0fbfbcc2484c
    worf - local  active  type: APPLICATION
  bff3b6ca-8be7-441c-823e-c36f1a61d41e
    litellm.cdot.io - no guardrails - REST API  active  type: APPLICATION
  f2953fa2-943c-47aa-814d-0f421f6e071b
    AWS Bedrock - Claude 4.6  active  type: MODEL
```

Each target shows its UUID, name, status (`active`/`inactive`), and type (`APPLICATION`, `MODEL`, `AGENT`).

## Create a Target

Targets are created from a JSON configuration file. The file must include `name`, `target_type`, and `connection_params` with the correct request/response format for your API.

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
      "choices": [
        {
          "message": {
            "content": "{RESPONSE}"
          }
        }
      ]
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

```
  Prisma AIRS — AI Red Team
  Adversarial scan operations

  Target created: 202c6988-d699-4dda-8f56-be7cc6d17136


  Target Detail:

    UUID:   202c6988-d699-4dda-8f56-be7cc6d17136
    Name:   docs-example-target
    Status: inactive
    Type:   APPLICATION
```

The target starts as `inactive` until validated. Use `--validate` to test the connection on creation:

```bash
airs redteam targets create --config target.json --validate
```

## Get Target Details

Inspect a target's full configuration including connection parameters:

```bash
airs redteam targets get 89e2374c-7bac-4c5c-a291-9392ae919e14
```

```
  Prisma AIRS — AI Red Team
  Adversarial scan operations


  Target Detail:

    UUID:   89e2374c-7bac-4c5c-a291-9392ae919e14
    Name:   litellm.cdot.io - no guardrails - REST APIv2
    Status: active
    Type:   APPLICATION

    Connection:
      api_endpoint: https://litellm.cdot.io/v1/chat/completions
      request_headers: [object Object]
      request_json: [object Object]
      response_json: [object Object]
      response_key: content
      curl: curl \
  "https://litellm.cdot.io/v1/chat/completions" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer **********" \
  --data '{"model":"mistral-7b","messages":[{"role":"user","content":"{INPUT}"}],"max_tokens":256}'
```

!!! tip "Curl command"
    The AIRS API generates a ready-to-use `curl` command for validated targets, with credentials masked.

## View Target Profile

Targets that have been profiled contain detailed context about the AI application — system prompts, tools, capabilities, and background:

```bash
airs redteam targets profile <uuid>
```

The profile includes:

- **Target background** — industry, use case, competitors
- **Additional context** — system prompt, base model, supported languages, accessible tools
- **Profiling status** — `COMPLETED`, `IN_PROGRESS`, or `null` (not yet profiled)

!!! info "Profiling"
    Profiling is initiated from the AIRS console. Prisma AIRS CLI can read the profile but profiling itself runs server-side.

## Update a Target

Update an existing target by providing a JSON file with the fields to change. The AIRS API requires `target_type` and full `connection_params` on every update.

```bash
airs redteam targets update <uuid> --config updates.json
```

Add `--validate` to re-test connectivity after updating:

```bash
airs redteam targets update <uuid> --config updates.json --validate
```

## Probe a Connection

Test a target connection without saving it. Useful for validating API credentials before creating a target:

```bash
airs redteam targets probe --config connection.json
```

The probe sends a test message and returns the raw response from the target API.

## Delete a Target

Remove a target that is no longer needed:

```bash
airs redteam targets delete 202c6988-d699-4dda-8f56-be7cc6d17136
```

```
  Prisma AIRS — AI Red Team
  Adversarial scan operations

  Target 202c6988-d699-4dda-8f56-be7cc6d17136 deleted.
```

!!! warning
    Deleting a target is permanent. Scans that reference the target will retain their results, but no new scans can be launched against a deleted target.

## JSON Config Reference

### Target Create/Update Fields

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
