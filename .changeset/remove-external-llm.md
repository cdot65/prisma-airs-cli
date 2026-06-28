---
"@cdot65/prisma-airs-cli": major
---

Remove all external-LLM functionality. Custom topic guardrail generation is now fully agent-driven (AGENTS.md / CLAUDE.md), so the LLM provider layer is no longer needed.

**Breaking changes:**

- Removed the `airs runtime profiles audit` command (it used an LLM to generate test prompts).
- Removed the LLM provider layer and the `audit` library exports (`runAudit`, `computeTopicAuditResults`, `computeCompositeMetrics`, `detectConflicts`, `buildAuditReportJson`, `buildAuditReportHtml`, and the `AuditEvent` / `AuditResult` / `TopicAuditResult` / `ConflictPair` types). `ProfileTopic` is retained (now exported from the AIRS types).
- Removed the orphaned run-report library exports (`buildReportJson`, `buildReportHtml`, and the `ReportOutput` / `RunDiff` / `MetricsDelta` / `TestDetail` / `RunSummary` / `IterationSummary` types).
- Removed the `llmProvider` / `llmModel` config fields and the LLM provider credential settings (`ANTHROPIC_API_KEY`, `GOOGLE_API_KEY`, `GOOGLE_CLOUD_*`, `AWS_*`, `LLM_PROVIDER`, `LLM_MODEL`). AIRS scanner + management credentials are unchanged.
- Dropped the `@langchain/*` and `@anthropic-ai/vertex-sdk` dependencies.
- Removed the LLM Providers documentation section.
