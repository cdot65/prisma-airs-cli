# Signal Brief pinned SDK capability matrix

Phase 0 implementation research, 2026-11-02. Dependency remains crates.io
`scm-sdk =0.2.4`; inspected registry source, not assumptions about a newer checkout.
This is proposed behavior until each corresponding implementation chunk updates
DESIGN and COMPATIBILITY with executable tests.

| Source | Typed method / fields | Meaning and limits |
| --- | --- | --- |
| Application inventory | DashboardClient::applications_overview | Relative time interval/unit, limit/offset; application IDs and literal metadata name buckets. |
| Session inventory | DashboardClient::sessions_overview | Relative window and pagination; identity app ID + exact app name + session ID. |
| Session events | DashboardClient::session | Requires all three identities; session_actions supplies scan_id and numeric scan_sub_req_id; paginated. |
| Transaction detail | DashboardClient::session_transaction | App/session/scan/sub-request identity; detector breakdown and observed profile, not full prompt content. |
| Scan results | Scanner::query_by_scan_ids | 1–5 UUID IDs; unordered ScanIdResult rows, optional req_id/result, result.report_id. |
| Detector reports | Scanner::query_by_report_ids | 1–5 opaque report IDs, each <=1024 bytes, no comma/control characters; ThreatScanReport identities and detection_results. |
| Snippets | DSDetailResult | pi_snippets, dlp_snippets.snippets, dbs_snippets, tc_snippets; optional and possibly empty. |
| Profiles / associations | ManagementClient::profiles, customer_apps | Current configuration; no proof of historical enforcement or application linkage without explicit identity evidence. |

ScanResponse.category/action and DetectionServiceResult.verdict/action are separate
observations. A report may contain malicious PI and benign DLP. Do not count benign
rows as violations or replace historical scan enforcement with detector actions.
Unknown fields/states are not benign; unavailable snippets are not empty payloads.

`tests/signal_sdk.rs` exercises an independent synthetic session -> scan-result ->
report chain against the loopback fixture with the pinned SDK. It establishes
compatibility with that modeled response, not live qualification. In particular,
numeric req_id and scan_sub_req_id are distinct fields: equality is only usable
when explicit returned identities agree and the association is unambiguous. A
missing request ID is never zero. Sync/async responses without sufficient supplied
identities remain unresolved. Scanner lookup only accepts UUID scan IDs; opaque
dashboard scan IDs are retained but cannot be sent to that method.

No legacy ScanLogsClient query, stored scan-content read, new scan submission,
customer evidence, real credentials, SDK edits or non-loopback calls are required.
No new real-service workflows have been qualified by this synthetic proof.

## Proposed contract and coverage ledger

The complete proposed grammar, bounds, privacy, stage reservations and strict rules
are in ../SIGNAL_BRIEF_IMPLEMENTATION_PLAN.md sections 3–4. They supersede the original
prompt's additive-command preference through the user's approved scope interview.

| Change | Delivery chunks | Executable coverage to add |
| --- | --- | --- |
| Evidence schema, identities and canonical digest | 1A | round trips, invalid graphs, absent vs zero, duplicate JSON keys, digest tampering |
| Shared masking including complete DLP withholding | 1B | planted secrets on every surface, repeated sensitive span across snippet families |
| Offline report and input preflight | 1C | nonexistent tenant, malformed input, zero network/client construction |
| Grouped analysis, monitoring interpretation | 2A | action-level conflicts, aligned denominators, current vs historical policy |
| Replacement report HTML | 2B, 4A | escaping/CSP/accessibility; explicit dNN comparison with frozen TS HTML |
| Working scan-logs query | 3A | paging, scope, partial-source output; dNN comparison with TS failure stub |
| Direct results/reports | 3B | scan-only tenant, missing IDs, batching, masked output; TS command rejection |
| Bounded sampled enrichment | 3C | deterministic sampling, starvation, deadlines and actual request attempts |
| Private no-replace bundle | 4B | race, sync/unsupported-primitive failure, coherent artifacts |
| Markdown review packet and import | 5A/5B | binding, injection, attribution, safe offline round trip |

Allocate unused divergence numbers when implementation lands. Preserve existing
historical rows, updating status/references with the matching executable coverage.
Every observable chunk updates its own DESIGN/COMPATIBILITY rows and tests; phase 6
only reconciles cross-cutting documentation. No release milestone completion is
claimed: user-run live qualification and publication remain outside this task.

Phase3 verified narrower window bounds: applications_overview intervals1/7/30/60,
units hour/day/days; application detail intervals7/30/60days. Default24hours can
be converted exactly to1day for overview, but not for detail token telemetry.
Unsupported windows create source gaps; no SDK bypass or approximate window query.
