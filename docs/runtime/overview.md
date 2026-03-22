---
title: Runtime Security
---

# Runtime Security

Runtime Security is the core module of Prisma AIRS CLI for real-time prompt scanning, configuration management, guardrail generation, and profile audits.

## What's in This Section

<div class="grid cards" markdown>

-   :material-magnify:{ .lg .middle } **Prompt Scanning**

    ---

    Scan individual prompts or bulk-scan from files against AIRS security profiles.

    [:octicons-arrow-right-24: Prompt Scanning](scanning.md)

-   :material-cog:{ .lg .middle } **Configuration Management**

    ---

    Full CRUD for security profiles, custom topics, API keys, customer apps, deployment/DLP profiles, and scan logs.

    [:octicons-arrow-right-24: Config Management](config-management.md)

-   :material-refresh-auto:{ .lg .middle } **Guardrail Generation**

    ---

    LLM-driven iterative refinement loop that generates, tests, and improves custom topic guardrails.

    [:octicons-arrow-right-24: Guardrail Generation](guardrails/overview.md)

-   :material-clipboard-check:{ .lg .middle } **Profile Audits**

    ---

    Evaluate all topics in a security profile at once with per-topic metrics and cross-topic conflict detection.

    [:octicons-arrow-right-24: Profile Audits](profile-audits.md)

</div>

## Authentication

Runtime scanning requires a Scanner API key (`PANW_AI_SEC_API_KEY`). Configuration management requires Management API credentials (`PANW_MGMT_CLIENT_ID`, `PANW_MGMT_CLIENT_SECRET`, `PANW_MGMT_TSG_ID`). Guardrail generation and profile audits require both, plus an LLM provider key.
