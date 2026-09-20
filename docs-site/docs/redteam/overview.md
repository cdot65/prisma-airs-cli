---
title: AI Red Teaming
---

# Red Team Scanning

Prisma AIRS CLI integrates with Palo Alto Prisma AIRS AI Red Team to run adversarial scans against configured targets. This provides a second layer of validation beyond the guardrail refinement loop's synthetic tests.

## Overview

The `airs-cli redteam` command group provides full access to Red Team operations:

- **Scan** -- launch static, dynamic, or custom prompt set scans
- **Status** -- monitor running scans
- **Report** -- view results with severity breakdowns and attack details
- **Judge** -- compute an independent attack success rate with TypeSafe Jev ([`redteam judge`](../cli/redteam/judge.md))
- **List** -- browse recent scans
- **Targets** -- full CRUD on red team targets (create, get, update, delete, probe, profile, validate-auth, metadata, templates)
- **EULA** -- check, view, and accept the Red Team end-user license agreement
- **Instances** -- create, read, update, and delete Red Team compute instances
- **Devices** -- create, update, and delete devices attached to instances
- **Registry Credentials** -- fetch container registry tokens for Red Team infrastructure
- **Prompt Sets** -- manage custom prompt sets (create, get, update, archive, upload CSV, download template)
- **Prompts** -- manage individual prompts within sets (add, list, get, update, delete)
- **Properties** -- manage custom attack property names and values
- **Categories** -- list available attack categories
- **Abort** -- stop a running scan

## Scan Types

| Type | Description |
|------|-------------|
| `STATIC` | Runs AIRS-maintained adversarial attack patterns from the attack library |
| `DYNAMIC` | Goal-driven multi-turn attacks using an adversarial agent |
| `CUSTOM` | Runs your custom prompt sets (created with `airs-cli redteam prompt-sets create` / `upload`) |

## Sub-pages

- [Environment Dashboard](../cli/redteam/dashboard.md) — CLI 5.1.0 read-only HTML/Markdown report with actual verified CLI output
- **[End-to-End Walkthrough](end-to-end-walkthrough.md)** -- tutorial: onboard a target, run a STATIC scan, pull the report, with every command + response
- [Running Scans](scanning.md) -- launch scans, monitor progress, view reports
- [Judge scan results with TypeSafe Jev](../cli/redteam/judge.md) -- 7.1.4 setup, dry run, bounded judging, replay and ASR interpretation
- [Managing Targets](targets.md) -- CRUD operations for red team targets, auth validation, metadata, templates
- [EULA & Infrastructure](infrastructure.md) -- EULA acceptance, instance management, devices, registry credentials, network broker channels
- [Prompt Sets & Prompts](prompt-sets.md) -- manage custom prompt sets and individual prompts

:::tip[Exact command syntax]
Every red team command with options and example output lives in the
[CLI Reference](../cli/redteam/scan.md).
:::

## Authentication

Red Team API operations use `mgmtClientId`, `mgmtClientSecret` and `mgmtTsgId`
from the selected CLI tenant. Set them through [tenant configuration](../cli/tenant.md);
credential environment variables are ignored. The same `mgmtTokenEndpoint` serves
management authentication across product groups. Optional `redTeamDataEndpoint` and
`redTeamMgmtEndpoint` settings change API destinations, not authentication.

Live `redteam judge` calls additionally use the tenant's `typesafeApiKey`. A local-file
dry run or replay needs no credentials; `--job` still requires management credentials
to fetch the scan. This distinction applies to the CLI; the library example below
passes its own configuration explicitly.

## Library API

The `SdkRedTeamService` and `SdkPromptSetService` classes are exported for programmatic use:

```typescript
import { SdkRedTeamService, SdkPromptSetService } from '@cdot65/prisma-airs-cli';

const redteam = new SdkRedTeamService({
  clientId: process.env.PANW_MGMT_CLIENT_ID,
  clientSecret: process.env.PANW_MGMT_CLIENT_SECRET,
  tsgId: process.env.PANW_MGMT_TSG_ID,
});

// Target CRUD
const target = await redteam.createTarget({
  name: 'My Target',
  target_type: 'REST',
  connection_params: { api_endpoint: 'https://api.example.com' },
}, { validate: true });

// Scans
const job = await redteam.createScan({
  name: 'API Scan',
  targetUuid: target.uuid,
  jobType: 'STATIC',
});
const completed = await redteam.waitForCompletion(job.uuid, (progress) => {
  console.log(`${progress.status}: ${progress.completed}/${progress.total}`);
});
const report = await redteam.getStaticReport(completed.uuid);

// Prompt set management
const promptSets = new SdkPromptSetService({
  clientId: process.env.PANW_MGMT_CLIENT_ID,
  clientSecret: process.env.PANW_MGMT_CLIENT_SECRET,
  tsgId: process.env.PANW_MGMT_TSG_ID,
});

const ps = await promptSets.createPromptSet('My Set', 'Description');
await promptSets.addPrompt(ps.uuid, 'Test prompt', 'Should trigger');
await promptSets.uploadPromptsCsv(ps.uuid, new Blob(['prompt,goal\n"test","goal"']));
```
