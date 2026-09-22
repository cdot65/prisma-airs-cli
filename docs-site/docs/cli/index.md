---
sidebar_label: Overview
sidebar_position: 0
slug: /cli/
---

# CLI Reference

Auto-generated from the `airs-cli` command tree. Every command below lists its synopsis, options, and at least one input/output example.

All help screens sort commands and options alphabetically. Read commands share
the six-format and pagination contract described in
[Exit Codes & Output Streams](../getting-started/exit-codes-and-output.md).

In option tables, **Resolved** means the format is selected in this order: the command's
`--output`, the global `airs-cli --output`, `defaultOutput`, then the `pretty`
fallback. Captured examples that show pretty output without an explicit format assume no output
override is configured; add `--output pretty` to reproduce that presentation regardless of local
configuration. JSON and YAML examples always specify their format explicitly.

- [`airs-cli aigateway telemetry`](aigateway/telemetry.md)
- [`airs-cli aigateway resources and CRUD`](aigateway/resources.md)
- [`airs-cli aigateway workspaces`](aigateway/workspaces.md)
- [`airs-cli model-security groups`](model-security/groups.md)
- [`airs-cli model-security install`](model-security/install.md)
- [`airs-cli model-security labels`](model-security/labels.md)
- [`airs-cli model-security models`](model-security/models.md)
- [`airs-cli model-security pypi-auth`](model-security/pypi-auth.md)
- [`airs-cli model-security rule-instances`](model-security/rule-instances.md)
- [`airs-cli model-security rules`](model-security/rules.md)
- [`airs-cli model-security scans`](model-security/scans.md)
- [`airs-cli redteam abort`](redteam/abort.md)
- [`airs-cli redteam adapter`](redteam/adapters.md)
- [`airs-cli redteam categories`](redteam/categories.md)
- [`airs-cli redteam devices`](redteam/devices.md)
- [`airs-cli redteam eula`](redteam/eula.md)
- [`airs-cli redteam instances`](redteam/instances.md)
- [`airs-cli redteam languages`](redteam/languages.md)
- [`airs-cli redteam list`](redteam/list.md)
- [`airs-cli redteam network-broker`](redteam/network-broker.md)
- [`airs-cli redteam prompt-sets`](redteam/prompt-sets.md)
- [`airs-cli redteam prompts`](redteam/prompts.md)
- [`airs-cli redteam properties`](redteam/properties.md)
- [`airs-cli redteam registry-credentials`](redteam/registry-credentials.md)
- [`airs-cli redteam report`](redteam/report.md)
- [`airs-cli redteam scan`](redteam/scan.md)
- [`airs-cli redteam status`](redteam/status.md)
- [`airs-cli redteam targets`](redteam/targets.md)
- [`airs-cli runtime api-keys`](runtime/api-keys.md)
- [`airs-cli runtime bulk-scan`](runtime/bulk-scan.md)
- [`airs-cli runtime customer-apps`](runtime/customer-apps.md)
- [`airs-cli runtime deployment-profiles`](runtime/deployment-profiles.md)
- [`airs-cli runtime dlp dictionaries`](runtime/dlp/dictionaries.md)
- [`airs-cli runtime dlp filtering-profiles`](runtime/dlp/filtering-profiles.md)
- [`airs-cli runtime dlp generate`](runtime/dlp/generate.md)
- [`airs-cli runtime dlp patterns`](runtime/dlp/patterns.md)
- [`airs-cli runtime dlp profiles`](runtime/dlp/profiles.md)
- [`airs-cli runtime profiles`](runtime/profiles.md)
- [`airs-cli runtime resume-poll`](runtime/resume-poll.md)
- [`airs-cli runtime scan`](runtime/scan.md)
- [`airs-cli runtime scan-logs`](runtime/scan-logs.md)
- [`airs-cli runtime topics`](runtime/topics.md)

Utility commands are documented in the guides:

- [`airs-cli tenant`](tenant.md)
- [`airs-cli doctor`](../getting-started/quick-start.md#verifying-your-setup)
- [`airs-cli completion`](../getting-started/quick-start.md#shell-completion)
