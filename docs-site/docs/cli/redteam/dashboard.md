---
sidebar_label: dashboard
---

# redteam dashboard

:::note[Added in CLI 5.1.0]
This command uses SDK 0.27.0, including its GET quota method. Upgrade the CLI to 5.1.0 or newer
to use it. Earlier SDK 0.26.0 installations cannot supply GET quota; there is no POST fallback.
:::

Generate a read-only environment deliverable from seven SDK feeds: dashboard overview,
scan statistics, targets, GET quota, scans, network-broker statistics and adapters.
This does not replace `airs redteam report <jobId>`, which reports on an individual scan.

## Usage

```bash
airs redteam dashboard
airs redteam dashboard --output markdown
airs redteam dashboard --strict --output-file ./redteam-health.html
airs redteam dashboard --output markdown --output-file -
```

Install or update using `npm install --global @cdot65/prisma-airs-cli@5.1.0`.
When working from source, build the checkout and use `node dist/cli/index.js` in place of `airs`.
The package pins SDK 0.27.0; no local SDK link is needed.

| Option | Default | Behavior |
| --- | --- | --- |
| `--output` | `html` | `html` or `markdown`; independent of general terminal preferences |
| `--output-file` | Unique timestamped file in CWD | New path only; `-` streams the artifact to stdout |
| `--title` | Red Team environment report | 1–240 characters |
| `--max-pages` | `40` | 1–100 pages per inventory; 15 records per request |
| `--strict` | Off | Return 1 after writing when any source is incomplete |

Files use atomic, private (0600), no-clobber publication. Existing paths and symlinks are
refused. Status output uses stderr; stdout contains only an explicitly streamed deliverable.
HTML includes inline CSS/JavaScript and a hash-based content security policy, makes no remote
requests, supports priority filtering and printing, and remains readable without JavaScript.
Debug logging is refused before collection. Credentials are loaded read-only with the existing
Management OAuth workflow and Red Team endpoint overrides.

Exit 0 means collection/delivery succeeded, not that the environment has no risks. Exit 1 means
operational failure, all sources unavailable, or strict incomplete evidence. Invalid options exit 2.
No scans, adapter executions, configuration changes, or quota-consuming jobs are submitted.

## Actual CLI output — September 7, 2026

The following is the exact stdout captured from the built command, not a synthesized sample:

```bash
node dist/cli/index.js --quiet redteam dashboard --strict --output markdown --output-file -
```

The live workflow passed 4/4 E2E tests: default HTML, Markdown file/stdout, strict partial-page
behavior, and overwrite/debug guards. Seven sources were complete, including 23 scans across
two pages. Configuration bytes remained unchanged. Source statistics reported 19 scans;
the inventory had 19 completed and 4 aborted scans. These are independent observations,
not proof that the two feeds always reconcile. The local 24-hour scan-creation count was zero,
which does not mean no attacks, completions or other activity occurred during that window.

```markdown
# Red Team environment report

**Prisma AIRS AI Red Teaming**

Assessment: **Attention required**

Collection: 2026-09-07T23:13:18.182Z → 2026-09-07T23:13:20.352Z

Scan creation window (UTC): 2026-09-06T23:13:18.182Z → 2026-09-07T23:13:18.182Z

## What needs attention

### ATTENTION: Elevated risk reported

Evidence: 2 in the CRITICAL risk category.

Next step: Review affected targets in Red Team; the statistics window is server-defined.

Source: Statistics

### ATTENTION: Elevated risk reported

Evidence: 1 in the HIGH risk category.

Next step: Review affected targets in Red Team; the statistics window is server-defined.

Source: Statistics

### REVIEW: Confirm expected broker connectivity

Evidence: 1 of 7 configured channels are online.

Next step: Compare with intended channel schedules before investigating connectivity.

Source: Network broker

## Dashboard overview

Current dashboard snapshot

| Metric | Count |
| --- | --- |
| Targets | 8 |

## Dashboard scan statistics

Server-default window &#40;unverified&#41;

| Metric | Count |
| --- | --- |
| Scans | 19 |
| Targets scanned | 8 |
| Risk: CRITICAL | 2 |
| Risk: HIGH | 1 |
| Risk: MEDIUM | 1 |
| Risk: LOW | 4 |
| Status: IN&#95;PROGRESS | 0 |
| Status: COMPLETED | 19 |

## Target inventory

Collected targets by type; consult source completeness.

| Type | Count |
| --- | --- |
| APPLICATION | 8 |

## Quota

Remaining is shown only for finite quotas; these are not daily usage counters.

| Type | Allocated | Consumed | Remaining |
| --- | --- | --- | --- |
| static | 0 | 0 | Unlimited |
| dynamic | 50 | 0 | 50 |
| custom | 50 | 5 | 45 |

## Scan inventory and daily creation activity

Daily count covers collected records created in the stated UTC window, not scans completed or attacks executed.

| Metric | Count |
| --- | --- |
| Collected scans | 23 |
| Created in preceding 24 hours | 0 |
| Unusable or future creation timestamps | 0 |

## Current status of collected scans

All collected scans, independent of creation window.

| Status | Count |
| --- | --- |
| ABORTED | 4 |
| COMPLETED | 19 |

## Network broker

Not-online channels may be intentionally idle; this does not establish an outage.

| Metric | Count |
| --- | --- |
| Configured channels | 7 |
| Online channels | 1 |

## Adapter inventory

Configuration status only; scripts and variables are excluded.

| Status | Count |
| --- | --- |
| ACTIVE | 3 |

## Evidence and completeness

Complete means the source was collected within its budget, not that the environment is secure. Unavailable is not zero.

| Source | SDK method | Window | Status | Records | Pages | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| Overview | getDashboardOverview&#40;&#41; | Current dashboard snapshot | complete | 1 | 1 | Collection completed. |
| Statistics | getScanStatistics&#40;&#41; | Server-default window &#40;unverified&#41; | complete | 1 | 1 | Collection completed. |
| Targets | targets.list&#40;&#41; | Current paginated inventory | complete | 8 | 1 | Collection completed. |
| Quota | getQuotaSummary&#40;&#41; — GET | Current quota snapshot | complete | 3 | 1 | Collection completed. |
| Scans | scans.list&#40;&#41; | Paginated inventory; local 24-hour creation filter | complete | 23 | 2 | Collection completed. |
| Network broker | networkBroker.getChannelStats&#40;&#41; | Current channel snapshot | complete | 1 | 1 | Collection completed. |
| Adapters | adapters.list&#40;&#123; include&#95;target&#95;count: true &#125;&#41; | Current paginated configuration | complete | 3 | 1 | Collection completed. |

## Scope and limitations

- This is a read-only snapshot, not a security certification, availability SLA or numerical health score.
- Only scan creation timestamps are filtered locally to the preceding 24 hours. Current scan status is not a history of status changes.
- Dashboard statistics use server defaults with an unverified measurement window. They are not daily totals and need not match the paginated scan inventory.
- Targets and adapters are current configuration inventories, not configuration change logs. Broker channels not online may be intentionally idle; no outage is inferred.
- Risk categories are server-reported. Quota consumption is not reset or reinterpreted as daily usage; unlimited zero allocation is not exhaustion.
- Partial sources describe collected records only. Missing data is unknown, never zero. Independently read pages are not an atomic snapshot.
- Only aggregate counters and category labels are included. Target names, IDs, connection settings, scripts, variables, prompts and scan content are excluded.

Prisma AIRS CLI · Red Team report schema 1
```
