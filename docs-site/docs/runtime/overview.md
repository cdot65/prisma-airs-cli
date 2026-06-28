---
title: Runtime Security
---

# Runtime Security

Runtime Security is the core module of Prisma AIRS CLI for real-time prompt scanning, configuration management, and guardrail generation.

## What's in This Section

- **[Prompt Scanning](scanning.md)** — Scan individual prompts or bulk-scan from files against AIRS security profiles.
- **[Configuration Management](config-management.md)** — Full CRUD for security profiles, custom topics, API keys, customer apps, deployment/DLP profiles, and scan logs.
- **[Guardrail Generation](guardrails/overview.md)** — Atomic `runtime topics` commands (create, apply, evaluate, revert) that an external agent orchestrates to refine custom topic guardrails against a static prompt set.
- **[DLP](dlp/overview.md)** — Full CRUD over the DLP service: filtering profiles, patterns, profiles, dictionaries. Twenty commands, shared OAuth token cache.

## Authentication

Runtime scanning requires a Scanner API key (`PANW_AI_SEC_API_KEY`). Configuration management requires Management API credentials (`PANW_MGMT_CLIENT_ID`, `PANW_MGMT_CLIENT_SECRET`, `PANW_MGMT_TSG_ID`). Guardrail generation requires both.

:::tip[Exact command syntax]
Every runtime command with options and example output lives in the
[CLI Reference](../cli/runtime/scan.md).
:::
