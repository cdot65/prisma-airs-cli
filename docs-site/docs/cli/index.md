---
sidebar_label: Overview
sidebar_position: 0
slug: /cli/
---

# CLI Reference

Auto-generated from the `airs` command tree. Every command below lists its synopsis, options, and at least one input/output example.

All help screens sort commands and options alphabetically. Read commands share
the six-format and pagination contract described in
[Exit Codes & Output Streams](../getting-started/exit-codes-and-output.md).

In option tables, **Resolved** means the format is selected in this order: the command's
`--output`, the global `airs --output`, `defaultOutput` / `PANW_CLI_OUTPUT`, then the `pretty`
fallback. Captured examples that show pretty output without an explicit format assume no output
override is configured; add `--output pretty` to reproduce that presentation regardless of local
configuration. JSON and YAML examples always specify their format explicitly.

- [`airs aigateway telemetry`](aigateway/telemetry.md)
- [`airs aigateway workspace`](aigateway/workspaces.md)
- [`airs model-security groups`](model-security/groups.md)
- [`airs model-security install`](model-security/install.md)
- [`airs model-security labels`](model-security/labels.md)
- [`airs model-security models`](model-security/models.md)
- [`airs model-security pypi-auth`](model-security/pypi-auth.md)
- [`airs model-security rule-instances`](model-security/rule-instances.md)
- [`airs model-security rules`](model-security/rules.md)
- [`airs model-security scans`](model-security/scans.md)
- [`airs redteam abort`](redteam/abort.md)
- [`airs redteam adapter`](redteam/adapters.md)
- [`airs redteam categories`](redteam/categories.md)
- [`airs redteam devices`](redteam/devices.md)
- [`airs redteam eula`](redteam/eula.md)
- [`airs redteam instances`](redteam/instances.md)
- [`airs redteam languages`](redteam/languages.md)
- [`airs redteam list`](redteam/list.md)
- [`airs redteam network-broker`](redteam/network-broker.md)
- [`airs redteam prompt-sets`](redteam/prompt-sets.md)
- [`airs redteam prompts`](redteam/prompts.md)
- [`airs redteam properties`](redteam/properties.md)
- [`airs redteam registry-credentials`](redteam/registry-credentials.md)
- [`airs redteam report`](redteam/report.md)
- [`airs redteam scan`](redteam/scan.md)
- [`airs redteam status`](redteam/status.md)
- [`airs redteam targets`](redteam/targets.md)
- [`airs runtime api-keys`](runtime/api-keys.md)
- [`airs runtime bulk-scan`](runtime/bulk-scan.md)
- [`airs runtime customer-apps`](runtime/customer-apps.md)
- [`airs runtime deployment-profiles`](runtime/deployment-profiles.md)
- [`airs runtime dlp dictionaries`](runtime/dlp/dictionaries.md)
- [`airs runtime dlp filtering-profiles`](runtime/dlp/filtering-profiles.md)
- [`airs runtime dlp generate`](runtime/dlp/generate.md)
- [`airs runtime dlp patterns`](runtime/dlp/patterns.md)
- [`airs runtime dlp profiles`](runtime/dlp/profiles.md)
- [`airs runtime profiles`](runtime/profiles.md)
- [`airs runtime resume-poll`](runtime/resume-poll.md)
- [`airs runtime scan`](runtime/scan.md)
- [`airs runtime scan-logs`](runtime/scan-logs.md)
- [`airs runtime topics`](runtime/topics.md)

Utility commands are documented in the guides:

- [`airs config`](../getting-started/configuration.md)
- [`airs doctor`](../getting-started/quick-start.md#verifying-your-setup)
- [`airs completion`](../getting-started/quick-start.md#shell-completion)
