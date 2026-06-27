---
title: Runtime Security
---

# Runtime Security

Runtime Security is the core module of Prisma AIRS CLI for real-time prompt scanning, configuration management, guardrail generation, and profile audits.

## What's in This Section

- **[Prompt Scanning](scanning.md)** — Scan individual prompts or bulk-scan from files against AIRS security profiles.
- **[Configuration Management](config-management.md)** — Full CRUD for security profiles, custom topics, API keys, customer apps, deployment/DLP profiles, and scan logs.
- **[Guardrail Generation](guardrails/overview.md)** — LLM-driven iterative refinement loop that generates, tests, and improves custom topic guardrails.
- **[Profile Audits](profile-audits.md)** — Evaluate all topics in a security profile at once with per-topic metrics and cross-topic conflict detection.
- **[DLP](dlp/overview.md)** — Full CRUD over the DLP service: filtering profiles, patterns, profiles, dictionaries. Twenty commands, shared OAuth token cache.

## Authentication

Runtime scanning requires a Scanner API key (`PANW_AI_SEC_API_KEY`). Configuration management requires Management API credentials (`PANW_MGMT_CLIENT_ID`, `PANW_MGMT_CLIENT_SECRET`, `PANW_MGMT_TSG_ID`). Guardrail generation and profile audits require both, plus an LLM provider key.

:::tip[Exact command syntax]
Every runtime command with options and example output lives in the
[CLI Reference](../cli/runtime/scan.md).
:::
