# Runtime history implementation plan — independent review

## Review 1 — 2026-11-04

Reviewer: independent Codex instance `/root/phase6_review`.

Reviewed document: `docs/RUNTIME_HISTORY_IMPLEMENTATION_PLAN.md`, prepared
2026-11-04, SHA-256:

```text
35aef1a63079032156ca4ac365c0db807fa7900531df72ae08f0f5733909f374
```

**Score: 9/10 — plan accepted.** No plan-level blocker remains in this revision.
This accepts the delivery sequence and its prerequisite gates. It does not award
implementation phase scores, establish live API behavior, authorize publication,
or waive the A0/B0 contract work. Each implemented phase still requires its own
independent score of at least 9/10. Preserve this review if later revisions are
rescored; append subsequent assessments rather than replacing review history.

### Scope fidelity and release separation

The plan captures the agreed direction: management-visible application coverage,
explicit per-application scanner credentials, persistent normalized history,
automatic retention of already-received CLI results, resumable metadata sync,
selective examples, historical offline reports, profile observation history,
portable archives, indefinite retention with explicit pruning, and optional
scheduling. It preserves detector verdict versus administrator allow/block action
and the profile-rows/service-columns enablement matrix.

The 8.1/8.2 split is appropriate. A0–A3 address the existing live workflow and
credential/evidence gaps without introducing SQLite. B0–B6 introduce the local
archive and change report's default data source with explicit migration notes.
Keeping stable publication and user-run service qualification separate from
review scores avoids treating synthetic tests as proof of upstream behavior.

### Primary API evidence and remaining identity uncertainty

The linked [official May 2026 release notes](https://origin-docs.paloaltonetworks.com/prisma-airs/new-features/by-date/prisma-airs/may-2026)
support application-associated keys for scan/report access. They do not establish
an original-secret requirement after rotation or explain a particular missing
response. The revised plan correctly limits its claim.

The immutable local `scm-sdk =0.2.4` source confirms:

- `src/airs/scan/scanner.rs`: results and reports accept one to five IDs;
  scan IDs require UUIDs and report IDs can be opaque. Neither query accepts an
  application ID parameter; credential selection must happen before the call.
- `src/models/generated.rs`: `ApiKey.customer_app_id`, `ApiKey.cust_app`,
  `CustomerApp.customer_app_id`, and dashboard application identifiers exist,
  but their optional fields and names do not prove that every ID namespace is
  interchangeable. `CustomerAppWithKeys.customer_app_id` is required;
  `api_keys_dp_info` is optional and its entries expose names plus `auth_code`,
  not a typed key ID. Neither names nor raw key models should be used as a
  shortcut for identity proof; auth codes must never enter retained history.
- `src/airs/management/dashboard.rs`: session discovery/detail queries expose
  rolling interval/unit parameters, not arbitrary absolute start/end selectors.
- `examples/live_qualification.rs`: both sync and async receipts are queried with
  `query_by_scan_ids`; an async-only explanation is unsupported by this SDK.
- Typed session overview/action breakdowns support positive management detector
  evidence. They do not supply trustworthy scanner enforcement or an inferred
  report identity.

A0 must therefore produce a field-level identity/association table and refuse
unsupported joins before A1 routes requests. Names must not establish ownership;
unbound flat credentials must not be tried against discovered applications.
Synthetic ownership fixtures prove routing behavior, not actual service ownership
or rotation semantics. These are explicit prerequisites already present in the
plan, rather than unresolved implementation instructions to guess around.

### Current-code and migration feasibility

`src/clients.rs::build_scanner_bounded` currently builds one scanner from flat
tenant credentials. `src/tenant/config.rs` uses a strict 27-key schema and a
string/integer value model; named credential tables require a deliberate schema
extension, canonical writes, masked views, import handling, and parser-error
secret safety. A0/A1 correctly include that work and preserve valid old files.

`src/commands/scan.rs` and `src/bulk/engine.rs` already receive normalized or typed
results. Retention can consume those responses without additional HTTP calls.
Bulk checkpoints have their own failure/recovery contract and must remain
independent. A result can lack the complete dashboard event identity or canonical
cloud identity: B2 must retain it in the promised quarantine/observation path,
without fabricating app/session/subrequest values or losing the successful scan.

Existing evidence is versioned and digest-bound; current reports and bundles use
bounded projection and private no-clobber publication. The plan preserves legacy
replay, immutable originals and explicit legacy cloud association. B0/B5 must
choose a representation that does not silently force an unbounded historical
dataset through today's single-collection limits or rewrite old review bindings.

### Storage, privacy and archive safety

The SQLite design is feasible subject to the B0 prototype. It explicitly covers
bundled-library/MSRV/native-platform checks, private file creation, sidecars,
read-only readers, migrations, locks, disk-full failures and crash recovery.
No network wait inside a write transaction, no automatic repair/deletion, and no
reader-created databases are important acceptance requirements.

B0/B1 must demonstrate no filesystem mutations by readers while a writer uses
WAL, including missing/unsafe sidecars and checkpoint conditions. If a safe read
cannot proceed, an actionable refusal is acceptable; a read-only connection flag
alone is insufficient. Path validation must withstand replacement races and
cover temporary files, backups, WAL/SHM and export destinations.

The export format remains intentionally undecided. B0 must specify it before B4:
imports must treat content as data, not execute imported SQL, migrations,
triggers, extension-loading instructions or attacker-chosen database objects.
A bounded logical format is one feasible choice. Required validation includes
identity, versions, provenance, resource limits and current privacy projection.

The plan correctly keeps secrets in TOML and routing references in history. Its
projection must cover every retained metadata field and sanitized snippet before
insertion, including SQLite sidecars and temporary outputs. DLP/authentication
withholding, collision refusal and unknown semantics remain binding. Profile
snapshots must be normalized projections rather than raw policy/extension dumps.
The stated disk-encryption boundary does not promise application encryption.

### Sync, idempotence and report budgets

Separate discovery, result and example progress prevents one unavailable source
from blocking all ingestion. Atomic record/checkpoint commits, unfinished-page
tracking, overlap, late arrivals, conflict preservation and distinct scan versus
observation identities are the correct model.

Rolling API windows cannot prove arbitrary historical recoverability. B0/B3 must
define when a segment is observed, traversed, incomplete or irrecoverable without
assuming stable offsets or treating absent pages as covered zero. Imports cannot
advance an API cursor. A changed observation adds provenance without adding a
second scan; an identical observation must not inflate counts or archive growth.

The example policy remains bounded: ordinary sync issues zero report requests;
explicit enrichment defaults to five IDs, hard maximum ten, and at most two
actual sends across all applications and credential clients. No cross-app batch,
no SDK replay outside the shared counter, serial calls, persistent 429 cooldown
and bounded missing-report revisit are explicit. B3 must also define cooldown
scope and clock handling so another run or rotated key cannot bypass that policy.

### Historical UI and population correctness

The plan directly addresses the misleading zero-findings presentation by keeping
aggregate violated sessions visible beside unresolved scan coverage. Sessions,
scans, detector observations and API aggregate windows remain separate units.
Overlapping aggregate snapshots must not be summed into a historical time series.

B5 must prove count/chart/table/coverage agreement for every supported filter,
including unknown/conflicting observations and truncated detail exports. A filter
cannot claim full-population totals from capped rows. Profile posture uses only
an observation at or before period end, with age and change-time uncertainty.
Offline script/CSP, injection, keyboard and print tests are explicitly required.

The revised pruning contract is correct: deleting retained detail degrades its
completeness and rebuilds summaries; independent API aggregates retain their own
population. Pruned data cannot turn into a complete zero-event window.

### Delivery gates and test coverage

| Planned phases | Assessment |
| --- | --- |
| A0–A1 identity/configuration/routing | Sound, provided identity and grammar gates finish before routing code. |
| A2–A3 recovery/diagnostics/8.1 qualification | Correctly separates positive management evidence, unknown enforcement, aggregate populations and real-service uncertainty. |
| B0–B1 schema/durability | Sound, with safe SQLite and cross-platform feasibility required before adoption. |
| B2–B3 retention/resumable sync | Sound; no extra scans, independent bulk recovery, atomic progress, bounded enrichment. |
| B4–B5 portability/history reports | Sound; identity-bound import, prune coverage, legacy replay and filtered populations are explicit. |
| B6 scale/schedules/8.2 qualification | Sound; numerical benchmark thresholds must be fixed before acceptance, and schedules remain optional. |

Required fmt/Clippy/full parity, Node 24/frozen-reference integrity, documentation
contracts, sandbox/path guards, failure injection and independent phase reviews
are retained. Native Linux musl and macOS qualification, Rust 1.89, archive/version
checks and Linux/macOS interchange are present. One-million-scan testing includes
repeated observations, years of data, memory, size and bounded report timings;
the workload is a benchmark, not a retention ceiling.

The score is 9 rather than 10 because concrete identity grammar, SQLite binding,
schema, reader/WAL strategy, command errors and quantitative performance targets
are intentionally deferred to named prerequisite phases. That is acceptable for
this staged plan. None may be silently deferred again past its dependent phase.

Review activity was limited to this plan, contributor/design contracts, relevant
local code, immutable SDK source and the linked public release notes. No private
bundle, tenant credential, live API, SDK edit or feature implementation was used.

## Review 2 — 2026-11-04 — revised after Claude Code review

Reviewer: independent Codex instance `/root/phase6_review`.

Reviewed revision: Revision 2 of `docs/RUNTIME_HISTORY_IMPLEMENTATION_PLAN.md`.
SHA-256:

```text
5c3cb5a143d7cbe3b60f5cf1a348d32ce0a044d23e45becd2c4254a88ce2dd01
```

**Score: 9/10 — revised plan accepted.** No unresolved plan-level blocker found.
Implementation still requires the separate Claude Code plan gate and the stated
independent Codex phase gates. This is not an implementation acceptance score.

The first review remains above as the historical assessment. Its score was too
generous about deferring compatibility and identity questions to A0/B0. Claude's
7/10 review identified concrete decisions that needed to exist before those
phases: flat-credential migration, downgrade compatibility, receipt counting,
sync exclusion and the full shipped platform set. This review accepts the revised
commitments, rather than relying on the first score to dismiss that criticism.
The unsuccessful Claude assessment is preserved separately in
`RUNTIME_HISTORY_CLAUDE_REVIEW.md`.

### Reassessment of required corrections

| Finding | Revised commitment and assessment |
| --- | --- |
| P1 — existing flat credentials | A local explicit binding operation preserves flat settings, materializes endpoint/retries and supplies an actionable report remedy. Unbound required result coverage remains incomplete; strict-exit and upgrade differences are explicit. Resolved at plan level. |
| P2 — old binary compatibility | Named tables deliberately stay in the same TOML source. Old binaries refuse those files after an opt-in write; private no-clobber rollback snapshots and explicit transactional restore are required. Old-file reads do not migrate automatically. Resolved without pretending downgrade compatibility. |
| P3 — complete credential scope | A set contains one key or token, admitted endpoint, retries and explicit app association. Shared read/report policies can force retries to zero. Existing D48 input rules apply. Resolved. |
| P4 — nonwriting SQLite readers | DELETE rollback journals are selected; WAL is excluded. Readers do not mutate database or journal bytes, and refuse hot-journal recovery. B0 prototypes the precise invariant instead of choosing it later. Resolved. |
| P5 — partial receipt reconciliation | Partial receipts are uncounted observations. Promotion requires one supported full-identity candidate with compatible cloud/app/session/subrequest/report evidence; ambiguous candidates remain conflicts. Promotion cannot add a second counted scan. Resolved, with the exact predicate still a B0 prerequisite. |
| P6 — real rolling traversal limits | No absolute selectors/durable cursor are promised. Truncated backfill re-traverses the rolling head, persisted offsets are not continuation proof, drift yields bounded partial useful work, boundaries retain uncertainty, and backlog/traversed-as-of states are required. Resolved. |
| P7 — concurrent collectors | A private per-archive process lease precedes network work, contention fails promptly, and import/maintenance coordinate progress changes. Cooldown spans credential names/revisions within cloud/service scope. Resolved. |
| P8 — submission and polling selection | Named selectors cover scan/bulk/direct lookup. Named bulk state records credential reference/app/endpoint; old state retains flat behavior. Removed/rebound references refuse instead of falling back. A0 must freeze state version and recovery grammar. Resolved. |
| P9 — rotation and indefinite retries | One active secret per named set; deterministic references, no probing secret history. Both results and examples have bounded persisted revisits; attribution-backed rejection or removed binding defers eligibility. Missing alone proves no credential cause. Resolved. |
| Three shipped targets | A3/B0/B6 explicitly cover x86_64-musl, aarch64-musl and aarch64-apple-darwin, including SQLite C cross-compilation and arm64 execution natively or under a documented emulator. Musl performance is measured. Resolved. |
| Pruned content in SQLite pages | Every writer enables secure_delete, temporary data stays in memory, prune verifies markers absent from live free pages, and exports contain logical validated data rather than raw pages/SQL. Compaction locks/free-space costs and prior-backup limits are explicit. Resolved. |

I independently checked the release target set against `scripts/release.sh`:
both Linux musl architectures and macOS arm64 are shipped. The current strict
tenant parser indeed refuses unknown top-level schema entries, and the SDK
`ScanResponse` lacks full dashboard application identity. The revised compatibility
and provisional-observation rules address actual current constraints.

### Remaining phase acceptance requirements

These are concrete prerequisites already assigned by the revised plan, not
unresolved product choices or reasons to bypass a review gate:

- A0 must show exact field-level application identity proof. Customer-app IDs,
  dashboard IDs and key names remain separate until supported. No auth_code,
  api_key_last8, secret hash or name-based association may fill the gap.
- A0 must define how bind-default selects the effective flat credential when
  legacy key and token both exist, how old/new bulk state versions are read, and
  how an explicit recovery can avoid silently changing app scope. These must have
  compatibility, secret-safe parse-error and no-network tests before A1 ships.
- The rollback snapshot and live named-table write need a tested failure order:
  backup failure leaves the tenant intact; a successful backup followed by a
  failed write leaves a usable private original. Restore cannot overwrite a
  different tenant merely because a path or local nickname matches.
- B0/B1 must demonstrate DELETE-mode readers create no files or recover journals,
  prove private journals/temporary files from creation, and specify bounded busy
  waits. Concurrent short receipt-retention writes need coherent transactions
  even though the sync lease primarily excludes duplicate network collectors.
- B0 must make promotion eligibility stable under later contradictory imports.
  Previously attached receipt provenance must not create duplicate population
  counts or erase a newly discovered conflict. Retained metadata remains useful
  even when attribution cannot be established.
- B0/B3 must fix retry limits, lease/cooldown clocks, rolling boundary uncertainty,
  revisit cadence and falling-behind criteria. A read returning no rows cannot
  become an authentication diagnosis or permanently covered empty history.
- B4 must test that all deletion paths enable secure_delete, that logical exports
  exclude deleted content, and that separately preserved provenance contains no
  pruned snippets. Backups, filesystem snapshots and SSD forensic erasure remain
  outside the live-archive deletion promise.
- B5 must pin local/live flag applicability, optional --with-examples, 50,000
  summary-cell/10,000-detail-row/32 MiB limits, disabled-filter fallbacks and base
  no-JavaScript/print populations. No top-K sample may silently drive population
  charts. New HTML scripts require the planned explicit DESIGN/parity difference.
- A3/B6 must execute tests on every shipped target and retain fresh source/hash/
  package proof. A cross-compile alone does not qualify aarch64-musl execution.

The new --no-history escape hatch, one-warning-per-invocation rule, metadata-only
submission retention, logical non-SQL archive interchange and unsupported network
filesystem policy also close useful operational gaps. The 8.1/8.2 split and the
five/default, ten/maximum, two/actual-report-send budget remain intact. Ordinary
sync and report refresh still make no report requests unless examples are
explicitly requested. Session aggregates, attributed scans, detector observations
and partial receipts remain separate report populations.

No feature implementation, SDK edits, live API calls, private configuration,
credential access or private bundle inspection was performed for this re-review.
Only this review file was changed by the reviewer.

## Review 3 — 2026-11-04 — dual credentials and bounded storage operations

Reviewer: independent Codex instance `/root/phase6_review`.

Reviewed revision: Revision 3 of `docs/RUNTIME_HISTORY_IMPLEMENTATION_PLAN.md`.
SHA-256:

```text
9ee5be3c1147ad07f3831acb1558b6eca09fdbd010f7720a321c0f11a0452d77
```

**Score: 9/10 — revised plan accepted.** No remaining plan-level blocker found.
The separate Claude Code plan gate remains applicable; implementation phases
still need independent Codex acceptance at 9/10 or higher.

Claude's second assessment, preserved as 8/10 in
`RUNTIME_HISTORY_CLAUDE_REVIEW.md`, identified genuine gaps after Review 2. In
particular, the SDK does not choose one flat secret by client-side precedence,
and selecting DELETE journals required an actual contention bound. Review 2
underweighted those concrete requirements. Its score and text remain unchanged
as historical assessments; this acceptance is specific to Revision 3.

### N1 — binding a dual-header flat configuration

Resolved. I inspected the immutable SDK's `src/auth.rs` scan authentication
branch: when both values exist it sends both Authorization/Bearer and x-pan-token
headers. There is no evidenced client-side choice of one secret.

The plan now refuses bind-default on dual-secret input without `--use key|token`,
rejects selection of an absent kind without writing, and warns that a single-kind
named set may behave differently from the old dual-header request. A1 explicitly
tests those cases. It also documents snapshot-copy rather than live-reference
rotation behavior. A0 still must freeze grammar and exact errors, but no longer
has to invent the migration decision.

### N2 — compaction output and memory use

Resolved at plan level. Compaction uses an explicit private disk destination
through VACUUM INTO, not an archive-sized in-memory ordinary VACUUM. The plan
requires an empty 0600 regular output, identity/race checks, verification before
replacement, disk headroom, fsync and interrupted-swap recovery.

A shared/exclusive maintenance gate now covers all archive users during the
copy/validation/swap operation, in addition to the sync/writer lease. Readers
cannot retain or reopen an old database across the swap; compaction refuses
contention instead of evicting them. Maintenance has its own bounded deadline,
and B6 measures compaction and ordinary report peak memory separately.

B0/B1 must demonstrate the chosen binding can safely address the verified output,
define lock order and cleanup at every interruption point, and preserve the
original archive until validated publication succeeds. These are explicit
feasibility gates, not permission for a path-based overwrite fallback.

### N3 — rollback-journal contention and retention loss

Resolved. Report/history read transactions have a two-second monotonic ceiling
including lock wait, with cancellation during SQLite execution. They release
their transaction before rendering, output or user interaction. A query that
cannot meet the bound fails with a narrower-scope/maintenance instruction.

Automatic retention waits at most 250 ms total per batch for gate/database
contention, then skips that batch and counts the unarchived results in one
invocation warning. There is no hidden spool, resubmission or indefinite busy
retry. This preserves remote scan semantics while honestly exposing local loss.
The plan requires long-reader retention tests, observed read durations and
warning-count tests. B0 must fix coherent observation cutoffs before chunked
multi-query reporting; releasing locks cannot silently mix report populations.

### Additional closure and remaining gates

Revision 3 also makes the first named write's downgrade impact visible before
writing; defines private snapshot listing/deletion and packaging exclusions;
requires a frozen pre-change binary for downgrade refusal qualification; verifies
secure_delete=ON across the whole live file and absence of a committed journal;
and explicitly excludes physical erasure guarantees for old blocks/backups.

Partial receipts are separately visible and tested in both ingestion orders.
Later contradictory identity evidence invalidates prior unique attribution and
recomputes counts. Scanner-only history identity derives from the pinned TSG and
configured/default management environment without acquiring management secrets;
ambiguous custom environments make retention ineligible with one warning.

The earlier 8.1/8.2 boundary, app-identity proof requirement, cloud namespaces,
rolling-window uncertainty, logical non-SQL exports, filtered population rules,
three shipped target gates and five/default, ten/maximum, two/report-send policy
remain intact. No new implementation scope contradiction was found.

Acceptance still depends on executing A0/B0 before dependent code, demonstrating
safe SQLite opens/output replacement on all shipped targets, enforcing the stated
deadlines under real contention, and measuring memory/scale in B6. The plan now
states the policies those phases must prove. It does not certify those proofs in
advance or establish an upstream credential/rotation diagnosis.

This was a plan/source review only. No feature code, private config or bundle,
credential, live API, SDK edit or service qualification was used. Only this review
file was changed by the reviewer.
