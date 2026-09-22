---
title: Configuration Management
---

# Configuration Management

Prisma AIRS CLI exposes full CRUD over AIRS runtime configuration resources via `airs-cli runtime` subcommand groups. All config management commands use the selected tenant's SCM OAuth credentials (`mgmtClientId`, `mgmtClientSecret`, `mgmtTsgId`).

## Security Profiles

```bash
# CRUD
airs-cli runtime profiles list
airs-cli runtime profiles get <nameOrId>
airs-cli runtime profiles get <nameOrId> --output json
airs-cli runtime profiles create --name "My Profile" --prompt-injection block --toxic-content alert
airs-cli runtime profiles update <nameOrId> --prompt-injection alert --malicious-code block
airs-cli runtime profiles delete <nameOrId>
airs-cli runtime profiles delete <nameOrId> --force --updated-by user@example.com

# Clean up old profile revisions (keeps only latest per name)
airs-cli runtime profiles cleanup              # preview only
airs-cli runtime profiles cleanup --force      # delete old revisions
```

**`create`** requires `--name`. All protection flags are optional — omitted sections get AIRS defaults.

**`update`** uses read-modify-write: fetches the current profile, merges only the flags you specify, PUTs the full payload. Existing policy sections (including topic-guardrails) are preserved.

See the [CLI Reference — Profiles](../cli/runtime/profiles.md) for the full flag table.

## Custom Topics & Guardrail Optimization

```bash
# CRUD
airs-cli runtime topics list [--limit <n>] [--offset <n>] [--output <format>]
airs-cli runtime topics get <nameOrId> [--output pretty|json|yaml]
airs-cli runtime topics update <topicId> --config <json-file>
airs-cli runtime topics delete <topicId> [--force --updated-by <email>]

# Guardrail optimization (atomic commands for agent loops)
airs-cli runtime topics create --name <name> --description <desc> --examples <ex1> <ex2> [--output json]
airs-cli runtime topics apply --profile <name> --name <name> --intent <block|allow> [--output json]
airs-cli runtime topics eval --profile <name> --prompts <csv> --topic <name> [--output json]
airs-cli runtime topics revert --profile <name> --name <name> [--output json]
airs-cli runtime topics sample [--output-file <path>]
```

See [Guardrail Optimization](guardrails/overview.md) for details on the optimization loop and [`AGENTS.md`](https://github.com/cdot65/prisma-airs-cli/blob/main/AGENTS.md) for the agent protocol.

## API Keys

```bash
airs-cli runtime api-keys list
airs-cli runtime api-keys create --config apikey.json
airs-cli runtime api-keys regenerate <apiKeyId> --interval 90 --unit days
airs-cli runtime api-keys delete <apiKeyName> --updated-by user@example.com
```

## Customer Apps

```bash
airs-cli runtime customer-apps list
airs-cli runtime customer-apps get <appName>
airs-cli runtime customer-apps update <appId> --config app.json
airs-cli runtime customer-apps delete <appName> --updated-by user@example.com
```

## Deployment Profiles

```bash
airs-cli runtime deployment-profiles list
airs-cli runtime deployment-profiles list --unactivated
```

## DLP Profiles

```bash
airs-cli runtime dlp profiles list
```

## Scan Logs

```bash
airs-cli runtime scan-logs query --interval 24 --unit hours
airs-cli runtime scan-logs query --interval 168 --unit hours --filter threat
airs-cli runtime scan-logs query --interval 720 --unit hours --limit 100
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `PANW_AI_SEC_API_KEY` | Prisma AIRS API key for scan operations |
| `PANW_MGMT_CLIENT_ID` | Management API OAuth2 client ID (config management) |
| `PANW_MGMT_CLIENT_SECRET` | Management API OAuth2 client secret (config management) |
| `PANW_MGMT_TSG_ID` | Management API tenant service group ID (config management) |
