# B4/B5 prereview and B6 qualification preparation

Read-only audit of the shared checkout on 2026-11-04. No implementation changes,
heavy tests, live calls, credentials, private validation bundles or SDK edits.
These are grounded acceptance probes, not findings that unimplemented code fails.
No numeric score is assigned to B4, B5 or B6.

Independent blackbox acceptance probes are prepared in
`tests/support/history_transfer_adversarial.rs`. They cover actual CLI transfer
roundtrips, malformed-input refusal and removal of every live snippet copy.
The support file is deliberately not a Cargo test target while B4 is incomplete;
include it from the B4 executable test target once those commands are available.
Its initial isolated run confirmed valid fixture setup and failed at the absent
export command, rather than silently skipping unimplemented behavior.

### In-progress component review — 2026-11-04 22:49 UTC

The independent reviewer inspected the emerging transfer components without
building in the shared checkout. These are admission concerns to close before
acceptance, subject to source changes and the unfinished outer staging validator:

- `repository::validate_transfer_fact` must reject withheld/redacted application,
  name and session labels for full events, just as normal ingestion does. A
  nonempty privacy marker cannot restore exact identity after erasure.
- Fact validation must preserve the evidence model's intrinsic ownership and
  uniqueness checks: detection report belongs to its event, unique report/row
  pairs and unique snippet families. The recent snippet state/excerpt checks
  address one part; they do not establish those other invariants.
- The recursive metadata-key allowlist admits wrong-shaped supported keys, for
  example a `sync_work.descriptor_json` whose `identity.scan_id` is an array.
  Validate each retained metadata context against its actual schema before
  durable copying; final checksum validity cannot prove semantic validity.
- Transfer accounting currently adds all SQL text columns plus 1,024 bytes,
  while ingestion budgets encoded facts plus 1,024 bytes. Pin a boundary case
  where a fact admitted by ingestion remains exportable/importable despite its
  SQL identity/source columns, without relaxing the ordinary write bound.

No complete B4 score is assigned. The four blackbox probes, including a positive
re-encoded control followed by self-checksummed semantic corruption cases, now
compile in the isolated checkout. They still require the implemented commands
for execution and acceptance.

The emerging codec/IO/column/export component suite subsequently passes eight
isolated tests. Three independent direct admission probes fail as expected on
that same component snapshot: an erased app ID is admitted as a full fact, an
unowned report is admitted, and an array-valued scan ID is admitted inside work
metadata. The preserved failure log is
`/var/tmp/airs-9p1-9_jsua1s/b4-independent-component-red.log`. A prepared isolated
descriptor correction parses `work::Descriptor` and reuses its source-specific
identity validation; its focused positive/negative probe passes. Neither result
certifies the unfinished outer importer. Findings and probe source hashes are in
`/var/tmp/airs-9p1-9_jsua1s/b4-component-review.md`.

## B3 gate: verified, 9.2 assessment already exists

The recorded B3 independent acceptance is **9.2/10**. I independently checked:

- SOURCE-SHA256.json hashes to `645e910de5e757d66e8562c9512275eadeddd684d6cb735fe4b3a4893d97c4b7` and lists 492 files.
- source.tar.gz contains those exact 492 contents plus its identical manifest
  (493 regular members total), without unsafe paths or links. The extra manifest
  member explains why a raw dictionary equality against source entries is false.
- The five raw receipt hashes in RUNTIME_HISTORY_B3_QUALIFICATION.md match.
- workspace.log parses as 3,833 passed, zero failed, five ignored, 98 groups,
  zero parity skips; required-check JSON records zero exit codes.
- All three accepted CLI binary hashes match the dossier: native amd64-musl,
  emulated ARM64-musl and native Apple Silicon.
- At inspection, the only changes to frozen listed source were the three final
  qualification/review/phase-record documents. I did not rerun B3 tests or hash
  every platform test executable independently.

Therefore earlier snapshot failures do not reopen current B3 acceptance. Its
functional/emulation limits and lack of release/live/performance acceptance remain.

The old privacy-marker admission concern is addressed: schema.sql:90 requires
an explicit boolean in receipt binding JSON. database.rs:199/222 compares the
complete exact SQLite schema signature to the current schema, and :228 also
checks cloud/analysis/redaction metadata. An older draft without that CHECK is
refused, even though the unpublished numeric user_version remains 1. This is
refusal, not a claimed migration or support for old draft data.

## B4: exact remaining implementation chunks

At inspection, src/commands/runtime_history.rs has retry/status/coverage/stats,
legacy import-bundle, compact and recover; full export/import/prune are absent.
The external session is implementing B4; do not duplicate those files blindly.

### Transfer format and bounded export

Contract: RUNTIME_HISTORY_STORAGE_CONTRACT.md:345-366. Fixed UTF-8 JSONL header,
typed allowlisted records, footer count map and digest over exact header/record
bytes including newlines. Per-line maximum 1 MiB, total maximum 16 GiB.
Whole-archive export is explicit maintenance, not an ordinary two-second read.
Reader::maintenance (database.rs:341) already holds the writer lease and exclusive
gate, allowing coherent bounded chunks without mutable-row cutoff shortcuts.

Acceptance probes:

1. Empty and nonempty archives round-trip each normalized record kind, all
   observation/run links, receipt privacy state, conflicts, provenance and coverage.
2. Concurrent cooperating writer cannot cross export chunks; no network call,
   database creation, chmod, recovery or migration occurs merely to export.
3. Existing, symlinked, hard-linked, loose-mode and swapped output targets refuse
   without overwrite. Output is private from creation, no-clobber published,
   fsynced with directory durability. Interrupted export cleans owned staging only.
4. Exact byte hashing distinguishes LF/CRLF, missing final newlines, altered
   record order/counts and post-footer bytes. Never export SQL, schema objects,
   physical pages, whole SDK extensions or tenant configuration.
5. Key/relationship validation is bounded at 16 GiB: keeping every primary key
   or relationship in an unbounded Rust map is not a bounded decoder. Use an
   explicit bounded/external staging strategy, with its resource limits measured.

### Validation before import mutation

Header cloud namespace must be recomputed from its canonical descriptor, checked
against explicit expected TSG/environment and selected context before archive
mutation. A checksum is integrity, not authenticity: a malicious input can supply
its own valid digest. Do not trust imported materialized projections as truth.
Source relations are schema.sql:53-193 and :214-245; repository.rs:985 derives
normalized records/projections rather than accepting arbitrary SQL rows.

Acceptance probes:

1. Reject unknown format/schema/analysis/redaction versions, kinds and fields;
   duplicate keys; dangling references; invalid UTF-8/JSON; oversize lines/input;
   missing/duplicate footer; bad count/digest; forged or mismatched cloud identity.
   Rejection preserves an existing destination archive byte-for-byte and leaves
   no initialized archive for invalid input. Document/clean any owned staging.
2. Change/delete/replace external input after validation: import uses only the
   private, owned, verified, fsynced staged logical copy, never rereads that path.
3. Reapply current credential/payload safety before durable writes. If privacy
   removes a supplied identity/report constraint, preserve identity_withheld
   semantics; never interpret the removed constraint as absent-at-source and
   promote onto an unrelated existing bucket. Source values or hashes of erased
   private labels must not become durable join keys.
4. Validate/rebuild IDs and relations coherently if projection changes normalized
   content. Do not copy source event verdict/action/projection rows that contradict
   retained facts, or silently merge different cloud namespaces.
5. Interrupt after multiple committed batches, preserve a labeled partial import
   with verified transfer identity, then resume/reimport idempotently. Do not claim
   whole-archive atomicity. A changed transfer with reused identity must refuse.
6. Run interchange both ways across Linux/macOS; include same scan UUID in
   different exact app-name/session/subrequest buckets and meaningful conflicts.

### Selective prune: all copies and later resurrection

Critical current storage facts:

- repository.rs:985 stores the full serialized EventFact in
  event_facts.fields_json. EventFact contains Event.detections[].snippets.
- repository.rs:1015 stores partial facts in receipt_facts.fields_json; legacy
  partial evidence can contain snippets even though the B2 adapter strips them.
- repository.rs:1134 additionally stores snippets in examples.excerpts_json.
- cached_event at repository.rs:779 rebuilds from event facts AND currently
  promoted receipt facts; deleting only examples rows leaves bytes and can make
  a subsequent read/rebuild display the old snippet again.
- schema.sql:92 restricts event deletion while receipts remain promoted. The
  contract explicitly requires demotion/detachment and recomputation in the same
  prune transaction (STORAGE_CONTRACT:220).
- coverage_segments currently permits observed/traversed_as_of/partial/
  unavailable/falling_behind (schema.sql:193), so absence of a bespoke pruned enum
  does not excuse presenting a pruned interval as complete zero traffic.

Acceptance probes:

1. Seed distinct markers in an event fact, a partial receipt, a promoted receipt,
   multiple meaningful observation variants and the examples projection. Run
   examples-only prune; verify requested markers are absent from every current
   logical record and the entire live database bytes, including free pages/slack.
2. Rehydrate/rebuild/reconcile after prune, export, and import the result into a
   fresh archive. Removed snippets must not reappear through any path.
3. Keep metadata, identities, conflicts, observation provenance and independent
   API aggregates when only examples were selected. Preserve a truthful detail-
   removed state instead of claiming data was never collected. Decide explicitly
   whether later opt-in retrieval may refetch; ordinary sync must make no report
   call and cannot silently restore removed optional detail.
4. Full event prune demotes/detaches affected receipts before delete, leaves no
   orphan, recomputes eligibility and invalidates retained-detail coverage. A later
   new full fact can promote only through the complete supported predicate.
5. Preview is non-mutating; force selects exactly the same deterministic scope
   under the exclusive maintenance protocol. App/date cutoffs use UTC half-open
   semantics and must not remove out-of-scope examples, profiles or aggregates.
6. secure_delete=ON, bounded transactions, cancellation/partial outcomes, no
   surviving rollback journal after commit, and no privacy-erasure claims about
   old exports, raw backups, snapshots or SSD blocks.

## B5: historical report architecture and acceptance

Current runtime_report.rs:59-67 still loads a tenant and collects live data unless
from_evidence is supplied. Its replacement must not silently keep live behavior.
Evidence at signal/evidence.rs:147/197/207 caps event detail at 10,000; today's
signal/analyze.rs:52/59/170 derives totals from those in-memory events. Reusing it
with only the capped detail vector would falsely turn sampled/capped rows into
whole-population counts. A bounded full-cohort summary plane or versioned view
model is necessary; preserve existing sealed-evidence/review bindings and v1/v2
rules replay deliberately rather than uncapping legacy parsing by accident.

Acceptance probes:

1. No refresh: report builds no API client, reads one coherent SQLite snapshot
   with the shared two-second deadline including lock wait, captures data then
   releases the transaction/gate before HTML rendering/publication. Missing
   archive, missing coverage, covered empty scope and empty archive are distinct.
2. Validate all flag conflicts before tenant/archive/file access. Live-only
   timeout/concurrency/request/page controls require refresh; with-examples needs
   refresh; explicit max-reports needs both. from-evidence remains tenant-
   independent/offline and refuses archive/live controls.
3. Refresh shares the same archive network lease as sync. A racing scheduled sync
   cannot duplicate network work. Failed refresh may publish existing history only
   with stale/partial provenance and the required operational exit.
4. Use a >10,000-event cohort where capped detail has a different verdict/app/
   detector distribution from the full population. Every chart/filter/count must
   retain correct full-cohort values or explicitly disable unsupported whole-
   population filtering. Test exactly 50,000 summary cells and one over, 10,000
   detail rows and one over, and the 32 MiB final HTML ceiling.
5. Same UUID across distinct five-part event identities, absent/zero subrequest,
   unknown event time, contradictory timestamp/verdict/action and overlapping
   rolling session aggregates never become fabricated clean counts or sums.
6. Historical profiles/apps choose the latest observation link at/before period
   end, using profile_runs/application_runs (schema.sql:152-165), not merely the
   original row's observed_ms. Test A/B/A reobservation and out-of-order arrivals;
   never fill a past report with a future/current profile snapshot.
7. Strict evaluates requested historical primary coverage, not today's API
   availability or optional example gaps. Pruned coverage remains incomplete.
8. Current HTML is script-free (signal/html.rs:1/87, tests near :573). Interactive
   filters require explicit DESIGN/paired parity changes, a narrow hashed inline
   CSP script, escaped data and network-denial tests. No-JS and print retain the
   base-scope full summary/coverage. Test desktop/mobile, keyboard, print, script
   injection, zero fetches and no database path in output.
9. Private bundle no-clobber publication, evidence digest/review binding, prior
   rules compatibility, source privacy and reproducibility remain executable gates.

## B6 design preparation that can happen now

This section is a protocol outline, not B6 implementation, measurement or acceptance.
B4 and B5 must pass >=9.1 first. It is safe to prepare a metric schema, fixture
specification, threshold decision table and execution matrix without changing
product code or claiming the later phase has begun/completed.

Existing numerical contract gates, which must not be weakened after a run:

| Gate | Bound | Source |
| --- | --- | --- |
| Coherent 30/90-day report read at >=1M distinct scans | <=2 seconds including lock wait; timeout-and-narrow is not a substitute | STORAGE_CONTRACT:388-392; database.rs:19 |
| Ordinary write batch | <=200 records / 256 KiB; target <=100 ms gate hold | STORAGE_CONTRACT:296-305 |
| Automatic-retention contention | One cumulative 250 ms deadline | STORAGE_CONTRACT:296-307 |
| Interactive summary/detail/output | <=50,000 cells / <=10,000 rows / <=32 MiB HTML | IMPLEMENTATION_PLAN:392-400 |
| SQLite allocator | 256 MiB; not a total-process RSS bound | STORAGE_CONTRACT:55-65 |
| Compaction free space | Budget up to twice source archive size | IMPLEMENTATION_PLAN:433-445 |

Before the B6 benchmark is run, freeze which additional metrics are acceptance
criteria and their numerical limits: end-to-end rendering, report/compaction RSS,
ingest throughput, archive size per retained scan/fact and maintenance deadlines.
Metrics explicitly designated diagnostic still need values/units/conditions; a fast
prototype alone cannot substitute for the product gate. Do not pick thresholds
from the resulting product measurements after the fact.

Useful preparation artifacts (separate from product implementation):

- A versioned result JSON schema: source/fixture/binary digests, target/host/CPU/
  RAM/filesystem/storage, Rust/SQLite/features/profile, execution method, operation,
  requested/eligible/full/detail counts, bytes, milliseconds, peak RSS, exit/state,
  deadline and contention outcomes, failed-attempt links and threshold verdict.
- A deterministic fixture specification with >=1,000,000 DISTINCT scan UUIDs,
  multiple exact identities/apps across multiple years, identical duplicates,
  meaningful changed facts, unknown/conflicting fields, unlinked receipts,
  sparse/conflicting coverage, profile reobservations and bounded examples.
  The B0 16-app/1,095-day pattern is an available starting fixture, not a new
  retention ceiling. Assert distinct scans separately from event/fact counts.
- Frozen correctness expectations for 30/90-day windows, app filters, UTC edges,
  display truncation and cell-budget fallback before performance measurements.
- Separate ingest vs query vs render vs export vs compaction timings; product
  ingestion must exercise actual normalized repository logic, not only direct
  INSERTs into a simplified prototype schema.
- Proposed reproducibility protocol: retain every attempt, report cold and warm
  runs separately, record variance as well as central values, identify competing
  host load, and never present a selected successful repeat as the only attempt.
- Target matrix: native enterprise amd64 GNU plus the SHIPPED optimized amd64-musl
  CLI; native Apple Silicon; ARM64 native or clearly labeled QEMU functional
  execution. QEMU RSS/timing must not be advertised as native ARM performance.
- Explicit long-reader/retention tests under product load, compaction temporary
  storage and peak RSS measurement, reader deadlines and maintenance busy results.
- Package-manifest/layout/checksum/aliases/source-provenance protocol and optional
  metadata-only systemd/launchd examples specification: explicit paths, safe logs,
  network lease overlap prevention and recovery. Preparing examples is not
  installing a timer or implying unattended maintenance.

The existing B0 prototype (prototypes/runtime-history/src/main.rs:410-546) inserts
simplified benchmark facts directly and measures indexed summary SQL. Its README
explicitly excludes full repository semantics and product report/compaction RSS;
reusing its passing figures as B6 product acceptance would be invalid.

## Disjoint work available to the parent team

1. Finish independent review/integration of the prepared isolated ETXTBSY harness
   fix, avoiding overwrite of the external B4 session's input snapshot.
2. Update the earlier acceptance audit/Board records to acknowledge verified B3
   9.2 acceptance; preserve old failed snapshots as historical evidence.
3. Provide the B4 author this acceptance checklist or independently construct
   malformed-transfer/prune acceptance cases after coordinating exact file
   ownership. Avoid implementing a second competing export/import/prune module.
4. The explicitly planned directional live-harness follow-up is outside B4's
   files (live/writes.rs and its loopback self-tests). It can be implemented in
   isolation with the fixed prefix/ledger/resource caps and reviewed >=9.1, but
   remains user-run only for real APIs. Check ownership before touching it.
5. Prepare the B6 protocol artifacts above as design work only, then implement/
   measure B6 only after the preceding phase gates pass. Do not start B5 product
   implementation early merely because its design checklist exists.
