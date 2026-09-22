# Daily security review · Synthetic example

**Prisma AIRS AI Runtime Security**

Assessment: **Attention required**

Window: 2026-09-06T08:00:00.000Z → 2026-09-07T08:00:00.000Z

Rolling last 24 hours, UTC &#40;approximate for server-relative application queries&#41;. Collection: 2026-09-07T08:00:00.000Z → 2026-09-07T08:00:00.000Z.

## At a glance

- Sessions in collected app buckets: **1,300**
- Violating sessions: **51**
- Violation rate: **3.92%**
- Complete evidence sources: **7 / 7**

## What needs attention

### ATTENTION: Violating sessions observed

Evidence: 51 violating sessions across 2 application buckets in the collected daily data.

Next step: Review the highest-volume applications in SCM, confirm intended enforcement, and investigate unexpected activity. Violations are not proof of a successful attack.

Source: Daily application activity

### REVIEW: Timeout allows traffic: Staging availability

Evidence: An active profile explicitly sets inline-timeout-action to allow.

Next step: Review the availability-versus-enforcement tradeoff with the application owner; this is not evidence of a timeout or bypass.

Source: Security profiles

### REVIEW: Inactive profile: Retired experiment

Evidence: The latest returned revision is explicitly inactive.

Next step: Confirm this is intentional before assigning the profile to an integration.

Source: Security profiles

## Daily application activity

API-reported rolling one-day application buckets, ranked by violating sessions. Partial-source totals cover collected buckets only. Violating sessions are not necessarily blocked sessions.

| Application bucket | Sessions | Violating sessions | Violation rate | Registered ID match |
| --- | --- | --- | --- | --- |
| Customer support | 860 | 43 | 5% | Yes |
| Engineering assistant | 320 | 8 | 2.5% | Yes |
| Document search | 120 | 0 | 0% | Yes |

## Current security profiles

Latest returned revision per profile name. These are current settings, not a record of changes during the daily window.

| Profile | Revision | Active | Timeout action | Storage masking | Last modified (UTC) |
| --- | --- | --- | --- | --- | --- |
| Production protection | 8 | Yes | block | On in all returned configurations | 2026-09-06T08:00:00.000Z |
| Retired experiment | 1 | No | Unknown | Unknown | Unknown |
| Staging availability | 2 | Yes | allow | On in all returned configurations | Unknown |

## Registered application inventory

Current registered applications, separate from scan-metadata application buckets. Association counts do not establish key validity or deployment health. No key values or auth codes are included.

| Application | Environment | Cloud | Model | Key associations |
| --- | --- | --- | --- | --- |
| Engineering service | staging | azure | Example model | Unknown |
| Search service | production | gcp | Unknown | Unknown |
| Support service | production | aws | Example model | 0 |

## Collected session observations

Source: complete. Timestamp-eligible entries: 1,300. Violated status: 51. Missing timestamps: 0; outside window: 0. These are collected session counts, not scan actions or detector events. No scan content is fetched.

| Session status | Entries |
| --- | --- |
| passed | 1,249 |
| violated | 51 |

## Daily session chart

Independent chart totals: 1,300 sessions; 51 violating sessions. Do not force these counters to equal a separately paginated inventory.

| Bucket time (UTC) | Sessions | Violating sessions | Detector violations |
| --- | --- | --- | --- |
| 2026-09-07T07:00:00Z | 1,300 | 51 | 52 |

## Top applications by detector violations

Server-ranked subset for one day, not a complete application inventory. Detector violations may exceed the number of violating sessions.

| Application bucket | Detector violations | Detection types |
| --- | --- | --- |
| Customer support | 52 | pi: 52 |

## Daily detector severity trend

API-reported detector-policy events over the rolling day. Severity counts are preserved independently from distinct session counts; no previous-day comparison is inferred.

| Bucket time (UTC) | Critical | High | Medium | Low | Total |
| --- | --- | --- | --- | --- | --- |
| 2026-09-07T07:00:00Z | 0 | 0 | 52 | 0 | 52 |

## Evidence and collection coverage

Complete means pagination finished for that source, not that the environment is secure or that all traffic was ingested. Unavailable does not mean zero.

| Source / SDK method | Window | Status | Records | Pages | Collection notes |
| --- | --- | --- | --- | --- | --- |
| Daily application activity — dashboard.applicationsOverview | Rolling 1 day | complete | 3 | 1 | Collection completed. |
| Security profiles — profiles.list &#40;latest=true&#41; | Current configuration | complete | 3 | 1 | Collection completed. |
| Registered applications — customerApps.list | Current configuration | complete | 3 | 1 | Collection completed. |
| Daily session inventory — dashboard.sessionsOverview | Rolling 1 day | complete | 1,300 | 52 | Collection completed. |
| Daily session chart — dashboard.sessionsChart | Rolling 1 day | complete | 1 | 1 | Collection completed. |
| Top application violations — dashboard.topApplicationsViolations | Rolling 1 day; server-ranked subset | complete | 1 | 1 | Collection completed. |
| Daily violation trend — dashboard.applicationsViolationsTrend | Rolling 1 day | complete | 1 | 1 | Collection completed. |

## Scope, privacy, and limitations

- SYNTHETIC EXAMPLE: all application names, counts, and configurations are fabricated documentation fixtures, not live customer data.
- This fixture uses a 60-page budget to collect 1,300 sessions; CLI default is 40 pages and would explicitly mark that capped collection partial.
- This is a read-only operational review, not an uptime SLA, compliance attestation, or proof that attacks succeeded or were blocked. No health score is invented.
- Application activity uses the API’s rolling one-day window; each paginated request evaluates its own server-relative window. Collection is not a transactionally consistent historical snapshot. Ingestion may lag.
- Application buckets, session inventory, chart sessions, and detector violations are different measurements. They are not added together. Session inventory summaries use timestamp-eligible entries only; absent data is unknown.
- Application buckets are keyed by registered application ID plus the literal scan metadata.app&#95;name, which can differ from registered names. Do not sum session counts as unique users or unique tenant-wide sessions.
- Current configuration is not a configuration-change audit. Missing policy settings are unknown, not disabled. Inactive profiles can be intentional; current profile state may differ from the revision used by a historical scan.
- Daily charts, rankings and severity trends come from their own one-day endpoints. Rankings are a server-selected subset, not a complete inventory. Per-app token summaries and drill-downs require longer windows; no daily token usage or previous-day comparison is inferred.
- The legacy ScanLogsClient / scan-logs query path is broken and under refactor. This report uses the verified dashboard session APIs instead. It never automatically fetches transactions or stored scan content.
- Prompts, responses, user identities/IPs, API keys, auth codes, tenant IDs, and raw errors are omitted. Application/profile names and configuration metadata remain confidential; review before sharing.

Generated by Prisma AIRS CLI · Report schema 2
