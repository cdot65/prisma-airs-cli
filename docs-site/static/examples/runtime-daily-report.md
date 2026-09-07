# Daily security review · Synthetic example

**Prisma AIRS AI Runtime Security**

Assessment: **Attention required**

Window: 2026-09-06T08:00:00.000Z → 2026-09-07T08:00:00.000Z

Rolling last 24 hours, UTC &#40;approximate for server-relative application queries&#41;. Collection: 2026-09-07T08:00:00.000Z → 2026-09-07T08:00:00.000Z.

## At a glance

- Sessions in collected app buckets: **1,300**
- Violating sessions: **51**
- Violation rate: **3.92%**
- Complete evidence sources: **3 / 4**

## What needs attention

### ATTENTION: Violating sessions observed

Evidence: 51 violating sessions across 2 application buckets in the collected daily data.

Next step: Review the highest-volume applications in SCM, confirm intended enforcement, and investigate unexpected activity. Violations are not proof of a successful attack.

Source: Daily application activity

### REVIEW: Scan log detail: unavailable

Evidence: The API returned no record array. An empty object or absent collection does not establish zero activity.

Next step: Restore access or retry collection; do not treat missing data as zero. Increase --max-pages if the page budget was reached.

Source: Scan log detail

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

## Collected scan-log observations

Source: unavailable. Timestamp-eligible entries: Unknown. Tokens across eligible collected entries: Unknown. Missing timestamps: 0; outside window: 0. These are sample/collection counts, not tenant totals.

No rows available. Consult the source status before interpreting this as an empty result.

## Evidence and collection coverage

Complete means pagination finished for that source, not that the environment is secure or that all traffic was ingested. Unavailable does not mean zero.

| Source / SDK method | Window | Status | Records | Pages | Collection notes |
| --- | --- | --- | --- | --- | --- |
| Daily application activity — dashboard.applicationsOverview | Rolling 1 day | complete | 3 | 1 | Collection completed. |
| Security profiles — profiles.list &#40;latest=true&#41; | Current configuration | complete | 3 | 1 | Collection completed. |
| Registered applications — customerApps.list | Current configuration | complete | 3 | 1 | Collection completed. |
| Scan log detail — scanLogs.query &#40;read-only POST&#41; | Rolling 24 hours | unavailable | 0 | 1 | The API returned no record array. An empty object or absent collection does not establish zero activity. |

## Scope, privacy, and limitations

- SYNTHETIC EXAMPLE: all application names, counts, and configurations are fabricated documentation fixtures, not live customer data.
- This is a read-only operational review, not an uptime SLA, compliance attestation, or proof that attacks succeeded or were blocked. No health score is invented.
- Application activity uses the API’s rolling one-day window; each paginated request evaluates its own server-relative window. Collection is not a transactionally consistent historical snapshot. Ingestion may lag.
- Sessions, scan-log entries, text records, and API calls are different units. They are not added together. Log counters describe only collected, timestamp-eligible entries; absent data is unknown.
- Application buckets are keyed by registered application ID plus the literal scan metadata.app&#95;name, which can differ from registered names. Do not sum session counts as unique users or unique tenant-wide sessions.
- Current configuration is not a configuration-change audit. Missing policy settings are unknown, not disabled. Inactive profiles can be intentional; current profile state may differ from the revision used by a historical scan.
- Per-app token summaries and detector/severity breakdowns require 7/30/60-day windows, so they are not presented as daily metrics. No previous-day comparison or trend is inferred.
- Prompts, responses, user identities/IPs, API keys, auth codes, tenant IDs, and raw errors are omitted. Application/profile names and configuration metadata remain confidential; review before sharing.

Generated by Prisma AIRS CLI · Report schema 1
