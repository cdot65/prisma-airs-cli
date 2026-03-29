---
title: Configuration Management
---

# Configuration Management

Prisma AIRS CLI exposes full CRUD over AIRS runtime configuration resources via `airs runtime` subcommand groups. All config management commands require Management API credentials (`PANW_MGMT_CLIENT_ID`, `PANW_MGMT_CLIENT_SECRET`, `PANW_MGMT_TSG_ID`).

## Security Profiles

```bash
# CRUD
airs runtime profiles list
airs runtime profiles get <nameOrId>
airs runtime profiles get <nameOrId> --output json
airs runtime profiles create --name "My Profile" --prompt-injection block --toxic-content alert
airs runtime profiles update <nameOrId> --prompt-injection alert --malicious-code block
airs runtime profiles delete <nameOrId>
airs runtime profiles delete <nameOrId> --force --updated-by user@example.com

# Audit all topics in a profile
airs runtime profiles audit <profileName>
airs runtime profiles audit <profileName> --format html --output audit.html
```

**`create`** requires `--name`. All protection flags are optional — omitted sections get AIRS defaults.

**`update`** uses read-modify-write: fetches the current profile, merges only the flags you specify, PUTs the full payload. Existing policy sections (including topic-guardrails) are preserved.

See [CLI Commands — Profile Protection Flags](../reference/cli-commands.md#profile-protection-flags) for the full flag table.

## Custom Topics & Guardrail Optimization

```bash
# CRUD
airs runtime topics list [--limit <n>] [--offset <n>] [--output <format>]
airs runtime topics get <nameOrId> [--output pretty|json|yaml]
airs runtime topics update <topicId> --config <json-file>
airs runtime topics delete <topicId> [--force --updated-by <email>]

# Guardrail optimization (atomic commands for agent loops)
airs runtime topics create --name <name> --description <desc> --examples <ex1> <ex2> [--format json]
airs runtime topics apply --profile <name> --name <name> --intent <block|allow> [--format json]
airs runtime topics eval --profile <name> --prompts <csv> --topic <name> [--format json]
airs runtime topics revert --profile <name> --name <name> [--format json]
airs runtime topics sample [--output <path>]
```

See [Guardrail Optimization](guardrails/overview.md) for details on the optimization loop and `program.md` for the agent protocol.

## API Keys

```bash
airs runtime api-keys list
airs runtime api-keys create --config apikey.json
airs runtime api-keys regenerate <apiKeyId> --interval 90 --unit days
airs runtime api-keys delete <apiKeyName> --updated-by user@example.com
```

## Customer Apps

```bash
airs runtime customer-apps list
airs runtime customer-apps get <appName>
airs runtime customer-apps update <appId> --config app.json
airs runtime customer-apps delete <appName> --updated-by user@example.com
```

## Deployment Profiles

```bash
airs runtime deployment-profiles list
airs runtime deployment-profiles list --unactivated
```

## DLP Profiles

```bash
airs runtime dlp-profiles list
```

## Scan Logs

```bash
airs runtime scan-logs query --interval 24 --unit hours
airs runtime scan-logs query --interval 168 --unit hours --filter threat
airs runtime scan-logs query --interval 720 --unit hours --page-size 100
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `PANW_AI_SEC_API_KEY` | Prisma AIRS API key for scan operations |
| `PANW_MGMT_CLIENT_ID` | Management API OAuth2 client ID (config management) |
| `PANW_MGMT_CLIENT_SECRET` | Management API OAuth2 client secret (config management) |
| `PANW_MGMT_TSG_ID` | Management API tenant service group ID (config management) |
