# Runtime credential correctness and persistent history

Prepared 2026-11-04 from the user-approved scope interview. This is a delivery
plan, not a description of implemented behavior. Current source is 8.1.0-rc.3;
no implementation changes, version changes, live qualification, or publication
are authorized by this document alone. Existing implementation authorization and
the requirement for independent Codex phase scores of at least 9/10 remain the
delivery gates. Plan review is recorded separately in
RUNTIME_HISTORY_PLAN_REVIEW.md.

Revision 3 incorporates the independent Claude Code reviews requested after the
scope interview. Its initial 7/10 assessment and subsequent scores are preserved
in RUNTIME_HISTORY_CLAUDE_REVIEW.md. Implementation is additionally gated on a
Claude Code plan score of at least 9/10; independent Codex phase gates remain.

## Evidence and release boundary

Palo Alto's [May 2026 release notes](https://origin-docs.paloaltonetworks.com/prisma-airs/new-features/by-date/prisma-airs/may-2026)
describe scan/report access as scoped to the application associated with the API
key. This supports application-scoped credential routing. It does not establish
that retrieval after rotation requires the original secret, nor prove the cause
of missing results in a particular saved collection. Those remain qualification
questions; an empty response must not become an authentication diagnosis.

The pinned crates.io scm-sdk =0.2.4 exposes query_by_scan_ids and
query_by_report_ids, each taking one to five IDs. Current clients.rs supplies a
single scanner credential set from a selected tenant. Signal evidence is sealed
per collection; there is no SQLite store or dependency. The TOML schema is strict
and must be deliberately extended, including its canonical writer, readers,
masked views, import behavior and safety tests.

Deliver two separately qualified releases:

1. **8.1:** application-scoped scanner routing, usable management violation
   metadata, fair collection, and precise missing-evidence diagnostics. Keep the
   current live report workflow and five-part event identity, except the explicit
   unbound-credential strict-coverage migration described below. No SQLite dependency.
2. **8.2:** durable SQLite history, explicit resumable sync, scan-result retention,
   historical offline reporting, portable archives and optional schedule examples.
   This changes report's default data source and requires explicit upgrade notes.

Do not publish 8.1 or 8.2 merely because an implementation phase passes review.
Keep the existing signing and publishing pipeline; publication is a separate action.

## Agreed product contract

| Decision | Approved outcome |
| --- | --- |
| Collection scope | Every management-visible application; unavailable scanner details are explicit gaps. |
| Collection entry point in 8.2 | runtime sync; runtime report reads local history. --refresh explicitly syncs before rendering. |
| CLI-originated scans | Automatically retain normalized results already received by scan, bulk-scan and resume-poll; do not issue extra retrieval calls for persistence. |
| Retained content | Normalized metadata, bounded sanitized snippets, profile snapshots, provenance and coverage. No full prompts/responses, raw API bodies or credentials. |
| Scanner credentials | Multiple named sets per tenant, explicitly associated with application IDs in TOML. SQLite records references, not secrets. |
| Credential-set scope | Exactly one API key or API token, scanner endpoint, retry setting and explicit application association; no regional inference from a name. |
| Retention | Indefinite until explicit pruning; expose size, ages and selective pruning. Key expiry/rotation does not erase history. |
| Sync continuity | Durable progress, overlap for late records, deduplication and resumable incomplete work. Metadata coverage and detail enrichment advance separately. |
| Pinned SDK limitation | Cross-run continuation means requery/dedup and revisiting durable work, not resumable server offsets. Rolling windows have uncertain edges and shifting pagination. |
| Cloud identity | Pinned TSG within a cloud environment; aliases and rotated credentials share history. Another TSG selects separate history. |
| Bootstrap | First sync requests the recent 24 hours; older backfill is explicit and subject to API capabilities and availability. |
| Reports API | Ordinary sync makes no report requests. --with-examples enables strategic selection: five IDs by default, at most ten IDs and two actual report sends globally per invocation. |
| Scan persistence failure | Preserve the scan command's normal result and exit semantics; warn on stderr. Do not resubmit a scan. Sync persistence failures are operational errors. |
| Existing bundles | Explicitly import evidence.json with validated provenance, deduplication and coverage; legacy bundles need explicit cloud-tenant association. |
| Dashboard scope | Historical Signal Brief HTML. Existing runtime dashboard subcommands continue querying SCM directly. |
| HTML interactions | Offline date/application/detector/verdict/action filters within the exported range. Charts, counts, tables and coverage stay consistent. |
| Profile history | Matrix as of selected period end, observed change timeline, and supported historical revision associations. Observation time is not effective change time. |
| Portability | Versioned history export/import across Linux and macOS; preserve provenance and coverage, exclude credentials. |
| Scheduling | Optional hourly metadata-only systemd timer and launchd examples; installation remains opt-in. |
| Revisions | Retain meaningful changed observations, deduplicate identical observations, flag conflicts and count scans independently of observations. |
| Partial-identity receipts | Retain CLI receipts outside attributed scan totals until uniquely reconciled with supported full identity. Promotion cannot create a second counted scan. |
| At-rest protection | Standard SQLite in private directories (0700), database/supporting files 0600. Host disk encryption supplies encryption at rest. |
| Performance | Benchmark at least one million distinct scans per cloud tenant, multi-year retention and bounded 30/90-day reports; this is not a retention ceiling. |
| Local commands | runtime history for status/coverage, lookup, import/export and pruning. scan-logs remains an explicit live API interface. |
| Automatic retention opt-out | --no-history on scan/bulk-scan/resume-poll; one safe persistence warning per invocation, not per batch item. |

The existing malicious-verdict versus administrator allow/block distinction
continues unchanged. Malicious + allow can be deliberate monitoring. Unknowns,
conflicts, missing detail and partial traversals never become benign outcomes.

## Architecture and safety contracts

### Credential routing

Resolve the management tenant, exact application bucket and configured scanner
association before sending a scanner request. Keep registered customer-app
identity, API-key application association and dashboard's exact app-name buckets
distinct. Do not infer an association from display names or profile names. Phase
A0 must document which pinned typed fields connect these identities and what
remains unverified. Ambiguous associations remain unresolved.

Keep existing flat scanner credentials usable for existing explicitly selected
scan operations. Never use an unbound legacy credential as an automatic fallback
for management-discovered applications. Direct opaque-ID lookups need an explicit
credential selection or an unambiguous configured association; never try keys in
sequence to discover ownership. No cross-application batching. Route all HTTP
attempts through the existing plane-specific guarded clients and budget counters.

**8.1 migration is explicit.** Existing valid flat-only files remain readable and
existing submissions/direct lookups continue using the flat credential by default.
Live management-discovered report enrichment requires a binding. Provide a local
one-time `airs tenant scanners bind-default TENANT NAME --app-id APP_ID` operation
that copies the flat scanner selection into a named application-bound set while
preserving the original flat settings. It materializes the effective scanner
endpoint and retry setting. It makes no network request and asserts only the
operator-supplied association. Unbound reports retain management data and explain
the exact binding remedy; required scan-results coverage stays incomplete and
--strict exits 1. Document this intentional safety change, with upgrade notes,
a DESIGN section 12 row and paired strict-exit regression tests. Do not promise
identical strict results before the one-time binding.

If both flat airsApiKey and airsApiToken are configured, bind-default refuses
unless `--use key|token` explicitly chooses one. If only one is configured it
selects that secret; choosing an absent kind fails without writing. The current
SDK sends both headers when both flat values exist; the server's precedence is
not established. Binding one kind can therefore differ from legacy dual-header
behavior. A1 tests dual-secret refusal, each explicit selection, and absent-kind
selection. bind-default is a snapshot copy, not a live reference: later flat-key
rotation does not alter the named set. Help, upgrade notes and doctor make that
distinction visible without comparing secrets or matching secret fragments.

**Downgrade policy is explicit.** Named tables extend the same tenant TOML file;
no scanner sidecar configuration source is introduced. Older strict-schema
binaries cannot read a tenant after its first named-table write. This is an
intentional, opt-in configuration-format compatibility limit, not a transparent
downgrade. Ordinary new-binary reads do not upgrade files. Before the first named
write, create a private, no-clobber rollback snapshot under the same config lock
and transaction safety rules; a backup failure leaves the live file untouched.
Provide a documented explicit restore operation through Transaction. It requires
an explicit destination/tenant identity check, restores only the chosen config,
and explains that changes made after the snapshot are not in it. Do not silently
restore or erase named settings when an old binary is launched. Document
side-by-side Homebrew/npm/RC limitations, recommend isolated config roots for old
versions, and pin old-file/new-binary plus new-file/old-binary refusal in tests.
Backup paths and snapshots are private and never emitted with secret data.
The first bind-default/set operation is itself the format upgrade; its help and
safe diagnostic state the downgrade limit and snapshot location before writing.
Provide snapshot metadata listing and explicit deletion using the normal
confirmation/force conventions; snapshots contain secrets and never enter history
exports, diagnostics attachments or development packages. A0 pins their names and
recovery lifecycle. A3 validates refusal using a frozen pre-change Rust CLI
artifact; an isolated schema fixture is an earlier test, not its substitute.

Each named set contains exactly one secret type (key or token), its own admitted
scanner endpoint and retries (0–5), plus explicit application association. Endpoint
input follows the existing D48 write rule; no environment/regional inference or
management endpoint fallback. Lookup/report retry policy still overrides the set
to zero where required by shared budgets. Flat legacy dual-key/token precedence
remains the existing SDK behavior (both headers sent); it does not define named
precedence or prove server-side selection.

Add `--scanner NAME` to scan, bulk-scan, results and reports. An omitted selector
uses flat credentials for direct operations; a named-only tenant needs a selector.
For management discovery, association determines the credential; a selector must
not broaden the associated application scope. resume-poll records and checks the
submission's non-secret credential reference, application association and scanner
endpoint. Old bulk-state files retain flat-credential behavior. New named state
uses a versioned format; removed/rebound references or endpoint changes refuse
polling with an actionable message and never fall back to another credential.
Same-application secret rotation is an explicit config operation; record its
non-secret revision and handle rejected old work as below. A0 must finalize the
state-version and explicit recovery/override grammar before A1 implementation.

TOML continues to be the only configuration source. Extend it through
store::atomic::Transaction. Config readers remain read-only. Inventory and masked
config output expose names and associations only. Include every configured secret
and every acquired OAuth lease in the invocation's SecretSet. Accept new secrets
through private input files or stdin, never new secret-valued argv options.

Changing an association or rotating a credential affects future requests; retain
non-secret historical references. A 401/403, missing record, malformed record,
pending scan and unsupported join are different states. Respect status evidence
without inventing server reasons or printing response bodies.

One named set has one active secret, not a history of automatically tried secrets.
History references use the local set name/revision and a proven api_key_id only
when available; never use api_key_last8, auth_code, a secret fragment or a hash of
the secret to discover identity. Result enrichment as well as report enrichment
has bounded persisted retries. Missing alone is not proof of credential mismatch.
Explicitly revoked/removed associations or attributable authorization rejection
enter credential-ineligible/deferred work states; do not retry them on each sync.
A changed valid binding or an explicit operator reset may make work eligible again.
Record uncertain post-rotation access as unqualified; never promise that the new
secret can recover old results. A0/B0 define exact transitions and retry limits.

### SQLite ownership and filesystem layout

Place history under a documented private data root separate from the TOML config
store. Use existing XDG data conventions; make any explicit data-root override a
documented path control, never an environment credential/endpoint source. Define
the exact path and path-validation precedence in phase B0 before opening files.

Use one local archive per canonical cloud identity. Canonical identity includes
the TSG and a defined cloud-environment namespace; it must not depend on local
nickname, client secret, key bytes or display name. Known endpoint aliases and
custom environments require an explicit, tested identity policy. Do not silently
merge unknown environments or split history merely because a secret rotates.

Reject unsafe directories/files, symlinks, foreign ownership and loose modes.
Create private files from the outset, including rollback journals, migration
backups and temporary files. Existing loose files are refused rather than silently
chmodded by a reader. No in-project unsafe code. Evaluate a bundled SQLite Rust
binding against Rust 1.89, Linux amd64/arm64 musl and macOS arm64 before adoption.
Do not depend on an end user's system sqlite3 executable or library installation.

Make data-writing operations explicit in DESIGN. report/history readers do not
initialize a missing archive, run migrations or modify the TOML store. Qualify
read-only SQLite operation with concurrent writers and journals: selecting a
read-only open flag alone is not proof that SQLite creates no files. sync,
import, explicit maintenance and scan-result retention are the allowed writers.

Use SQLite DELETE rollback-journal mode for this release. Readers take shared
locks but do not change database bytes or create/update sidecar files. Accept
short writer/reader contention as the cost of this invariant. No WAL/SHM read-mark
exception is implied; WAL is out of scope. A reader encountering a hot journal or
an archive that requires recovery refuses with an explicit writer/maintenance
instruction. It never performs recovery, changes journal mode or repairs files.

Use transactional bounded writes and short locks; no network waits in an open
write transaction. Specify busy timeouts, writer contention, interrupt cleanup,
disk-full behavior and rollback-journal recovery ownership. Migrations are versioned,
transactional and validated; refuse unsupported newer schemas, preserve data on
failure, and provide an explicit maintenance path if a read requires migration.
Never recreate or delete an unreadable/corrupt archive automatically.

Bound each report/history read transaction to two seconds of monotonic elapsed time, including
its bounded lock wait; enforce cancellation/deadline during SQLite work. Fetch
bounded report data and release the transaction before HTML rendering. Do not
hold a read transaction across slow output, network work or user interaction.
If a consistent query cannot complete within the limit, fail with a narrower
scope/maintenance instruction instead of holding a shared lock indefinitely.
B0 defines a coherent observation cutoff and any chunking needed for a consistent
multi-query view; chunk boundaries cannot silently mix different populations.

Automatic retention waits at most 250 ms total per write batch for database/gate
contention. Busy outcomes skip that batch, count the unarchived results, and join
the one per-invocation stderr warning. There is no fallback spool and no hidden
scan retry. Sync/maintenance writes use their separately bounded busy policy.
B1/B6 must test a long concurrent report read against scan and bulk retention,
including the warning count and the read deadline. B6 reports peak report memory
and observed transaction durations and must demonstrate the stated bounds.

Supported archives reside on local filesystems. Refuse known NFS/SMB mounts;
document that live archives in network/cloud-sync directories are unsupported.
Use logical export/import to transfer data rather than concurrently sharing a
database directory. Filesystem detection cannot prove absence of a sync client.

### Records, observations and coverage

Preserve the existing event identity `(app_id, exact_app_name, session_id,
scan_id, sub_request)` inside its cloud namespace. Missing subrequest is not zero.
Do not merge same-named applications or same scan IDs across unsupported identity
relationships. Keep app discovery buckets and canonical app associations with
their evidence. Missing identities are quarantined, not guessed.

CLI receipts can lack app ID/name, session or subrequest even when they contain a
scan UUID. Store those as partial observations, excluded from attributed unique
scan totals, not as an additional fully identified scan. Require deterministic
promotion/reconciliation: within the same cloud namespace, match the exact scan
UUID, every supplied session/subrequest/report ID, any known explicit application
association and compatible provenance; require exactly one supported full-identity
candidate and no conflicting supplied fields. App association from the explicitly
selected named credential is request context, not a field invented in the SDK
response. Where available identity evidence does not support the join, keep the
receipt separately labeled and uncounted. Ambiguous/multi-bucket candidates remain
conflicts. Promotion attaches receipt provenance to the existing scan and
atomically removes provisional eligibility; it never counts both. B0 fixes the
exact predicate before ingestion code. No scan-ID-only joins across unsupported
application namespaces or guessed cross-bucket relationships are permitted.
Show unpromoted receipt counts as their own labeled population. Test both orders:
receipt before sync and receipt arriving after an already indexed scan (for
example a late resume-poll). A later conflicting identity invalidates any formerly
unique attribution; preserve provenance and recompute affected counts.

Store scans, meaningful observations, detector observations, sanitized examples,
configuration snapshots, source/aggregate observations, collection runs, coverage
segments, routing-reference provenance and sync work separately. Enforce foreign
keys and uniqueness with parameterized SQL. Preserve event time, observation time
and collection time independently, in UTC with half-open query intervals.

Repeated ingestion must be idempotent. Enrichment resolves unknown fields without
creating another scan. Contradictory positive observations remain attributable
conflicts; never silently let the last response overwrite historical truth.
Separate scan-level verdict/action from detector-level verdict/action and profile
configuration. Record schema, analysis and redaction versions for reproducibility.

API aggregate totals and locally observed scan counts are separate populations.
Do not sum overlapping query-window totals, equate sessions with scans, or turn
missing buckets into zero. Materialized summaries must be rebuildable from
retained facts and exclude duplicate observations. Completeness is tracked per
source, application, time segment and detail level; a successful sync exit alone
does not establish a complete historical interval.

### Sync and selective examples

Metadata collection uses the management plane across all visible app buckets,
with bounded fair scheduling. Scanner results enrichment uses only eligible
application credentials. Keep progress separately for discovery, scan results
and example reports so one unavailable plane cannot stall all ingestion.

Bootstrap 24 hours, then revisit incomplete work and overlap recent collection
for delayed records. Persist progress only with the records it acknowledges. Do
not advance a completeness watermark past an unfinished page. Persist stable
work descriptions, not an assumption that an API cursor remains valid forever.
Requery overlapping windows and deduplicate if resumable cursors are unsupported.

The pinned dashboard/session APIs have rolling interval/unit plus limit/offset;
they provide neither an absolute historical start/end selector nor a durable
cursor. Therefore truncated backfill restarts the rolling traversal head in the
next invocation, reusing saved observations and skipping already completed detail
work. Saved offsets are only within-run hints; never persist them as proof of
cross-run continuation or promise bounded catch-up. Larger rolling windows include
newer data and can repeatedly consume the traversal budget before reaching older
records. Expose a falling-behind state when budgets/arrival rate prevent progress.

For sync only, tolerate changing totals/offset drift by retaining valid unique
records, marking traversal incomplete and continuing bounded useful work. This
does not loosen strict response validation or change existing live sessions
command contracts. Structural malformed pages remain errors. Document the
rolling-window boundary uncertainty: record request start/end times and server
time when supplied, use conservative bounds, and overlap for clock skew plus
ingestion delay. Describe segments as traversed-as-of a collection time, not
permanently settled; late arrivals beyond overlap remain a declared limitation.
Exact overlap, skew allowance, revisit cadence and backlog criteria are B0 gates.

Backfill accepts only windows representable by actual pinned SDK requests. The
SDK does not thereby promise arbitrary historical server access. If a request
cannot address the requested period, record that limitation; do not silently
substitute a different period. Expired upstream data is an unrecoverable coverage
gap unless a genuine later import supplies it. Imported snapshots do not by
themselves authorize advancing an API traversal checkpoint.

Default sync does not call query_by_report_ids. Explicit example enrichment
reuses stored successes, prioritizes missing detector/app coverage and chooses
exact existing report IDs. Cap all apps together at five IDs by default, hard
maximum ten and two actual report sends. With app-separated credentials, this
may permit fewer than ten examples; disclose deferrals. Serialize report calls,
disable retries and stop on 429, honoring a persisted service cooldown across
scheduled runs. Record why candidates were skipped. Missing/failed examples have
bounded retry eligibility; repeated syncs must not loop on the same unavailable
IDs. No automatic full-content retrieval.

Acquire a private process-level lease for the canonical archive before any sync
network work, including report --refresh and explicit example enrichment. Use the
existing flock safety approach with a distinct private data-root lock; do not
hold the TOML transaction lock over network requests. Contention fails promptly
with exit 1 and an already-syncing diagnostic. SQLite busy timeouts are separate.
The lease protects network budgets and atomic work scheduling, and is released on
exit/interruption. Scope persisted report cooldown to canonical cloud/service
endpoint across credential names/revisions so rotating a key or changing an alias
does not bypass it. Import/maintenance/prune coordinate through the same writer
lease when altering progress/cooldown state. Cross-host service quotas cannot be
enforced by one local lease; recommend one scheduled collector per tenant archive.

Metadata and results have separate existing time/page/event/request ceilings;
document each command's applicability and resume behavior before implementation.
Cancellation leaves committed records valid and unfinished coverage explicit.

### Reports and archive lifecycle

In 8.2, report reads local history by default. A missing archive gives an
actionable sync instruction without network access. An empty archive, an empty
covered window and a window lacking coverage are distinct. --refresh performs a
bounded sync, then renders retained data with its honest coverage; failures retain
existing partial-output/strict-exit conventions. Explicit --from-evidence remains
tenant-independent and strictly offline, including when --refresh is also supplied
(an invalid combination rejected before file or tenant access).

In 8.2, --interval/--unit/--app-id scope local history and scope any explicit
refresh. --max-events caps exported detail with truncation disclosure; it does
not redefine the population of charts. --max-pages/--max-requests/--timeout/
--concurrency are live collection controls and require --refresh on report.
--max-reports requires explicit --with-examples plus --refresh on report;
--with-examples alone never triggers network access. --refresh is metadata/results
sync only unless --with-examples is explicitly supplied. Reject inapplicable flags
with exit 2 before tenant/archive access rather than silently ignoring them.
B0 fixes the complete applicability table, conflicts and exact error strings.

Keep the current default 24-hour report window unless explicitly changed in the
implementation contract. Add UTC start/end selection, longer intervals, and
30/90-day views. Partial coverage and freshness remain visible in HTML, evidence
and review packets. Strict mode evaluates the requested historical window's
required coverage, not today's API availability or optional example availability.

Offline filters operate only on the exported dataset and declare any row caps.
Bound detail exports and render trends from correctly scoped summaries; filtering
must not present capped rows as full-population totals. Either include the summary
dimensions required by every supported filter or clearly restrict the filter to
the detail view. No API/assets/network access, local database paths, or unbounded
archive embedding. Use escaped data, narrowly hashed CSP scripts, accessible
controls, keyboard support and printable fallbacks. Preserve the profile/service
matrix orientation and explicit-enable check marks.

Budget interactive summaries to at most 50,000 dimension cells and exported
detail to at most 10,000 rows, with a final 32 MiB HTML byte ceiling. If a requested
filter dimension exceeds that budget, disable the unsupported whole-population
filter and explain how to export a narrower range/application scope; keep the
unfiltered summary and declared bounded detail useful. Do not silently filter
charts from top-K or sampled rows. No-JavaScript and print views retain the base
scope's summary and coverage; scripts progressively enhance that view. Record
the intentional change from script-free HTML to hashed inline controls in DESIGN
and a section 12 parity difference. B0/B5 must pin byte/cardinality fallback tests.

Profile snapshots are observations. Select the latest observation at or before
the report period end, label its age, and never use a future snapshot to fill an
older period. A change timeline establishes differences between observations, not
the exact effective change time. Associate a scan with a policy revision only
when supported by evidence.

History exports are versioned, bounded and private, with validated identity,
schema/digest and provenance. Imports reject malformed/unsafe content before
publication, reapply current safety projection without corrupting identities,
deduplicate, preserve meaningful conflicts and do not merge cloud namespaces
implicitly. Legacy bundle imports require explicit target identity and preserve
the original bundle and digest. Keep legacy evidence/rules replay supported.

Provide history status/coverage, lookup, export/import, and prune. Prune supports
date, tenant and evidence-type selection, previews the affected counts and uses
explicit destructive-action confirmation/force conventions. Retain provenance
explaining removed detail so it does not appear never collected. No automatic
retention or deletion when tenant config, credentials or applications disappear.
Pruning invalidates the corresponding retained-detail completeness claims and
rebuilds affected summaries. Independently retained API aggregates keep their
own population/coverage; they do not imply that the underlying scans remain
available. A pruned interval must never look like a covered zero-event interval.

Enable secure_delete=ON (not FAST) on every writer before deletions. Ordinary
queries use memory-only temporary work within measured memory limits; this is
not a promise to compact an archive-sized database in RAM. Verify removed snippet
markers are absent from the entire current live database after prune, including
cell slack and free pages, and that no rollback journal remains after commit.
DELETE-mode rollback journals briefly contain original pages before unlinking;
their private mode does not imply physical erasure of previously allocated blocks.
Export logical validated records, never raw pages, SQL or executable schema objects.

Compaction is a separate explicit maintenance operation using VACUUM INTO a
private no-clobber destination inside the 0700 data root, not ordinary in-place
VACUUM under temp_store=MEMORY. Precreate/validate an empty regular 0600 output
with a held descriptor/identity, refuse races, and verify the compacted database
before replacement. Budget up to twice the source archive size in free disk and
measure compaction peak memory. Hold the sync/writer lease and an exclusive
maintenance gate across copy/validation/swap. All archive operations must honor
that gate: readers and ordinary retention use an existing private shared gate
without creating files and fail/skip promptly if maintenance owns it. Ordinary
sync holds the shared maintenance gate during archive access, never a SQLite
transaction over network work. Compaction refuses contention instead of evicting
readers. Fsync the new database and directory, atomically replace only the verified
source identity, and fsync the directory again; define interrupted-swap recovery
in B0. Readers cannot open a database during the swap. Temporary output modes,
cleanup, active-file identity checks and memory/disk bounds are B0/B1/B6 gates.
This explicit maintenance connection has its own bounded operation deadline;
the ordinary report/history read-transaction limit does not apply to compaction.

Pruning the live archive does not erase prior exports/backups or promise forensic
erasure from SSD/filesystem snapshots. Identify these boundaries in prune/backup
documentation and preserve explicit backup/snapshot deletion controls.

## Phases and delivery chunks

Every phase ends with executable evidence, documentation appropriate to that
phase and independent Codex review >=9/10. A low score stops progression until
findings are fixed and independently rescored. A plan score does not certify code.

### A0 — 8.1 capability and command contract

- Inspect immutable scm-sdk 0.2.4 types for app/key/dashboard identity links,
  scanner selection and supported result/report fields. Record contract evidence
  and unresolved live assumptions without reading real credentials.
- Specify exact named-credential TOML grammar, private-input management commands,
  legacy binding/rollback behavior, regional endpoint/retry scope, explicit
  submission/lookup selection, rotation and diagnostics. Define migration-free
  reading of old files, old-binary refusal of new tables and canonical writes.
- Freeze scan/bulk-scan/resume-poll selectors and versioned bulk-state reference
  compatibility, including explicit recovery after removed/rebound references.
  Credential references never contain auth_code or api_key_last8.
- Pin dual-flat binding selection, copied-default rotation behavior, safe snapshot
  lifecycle and first-write downgrade diagnostics.
- Add synthetic cross-app fixtures and explicit association cases. Update DESIGN
  and compatibility contracts with executable tests as behavior lands; future
  proposals remain labeled as future until implementation.

Gate: no unproven app-ID equivalence, no name-based routing, no unresolved
credential fallback policy. No SDK dependency change.

### A1 — 8.1 credential configuration and guarded routing

- Extend strict TOML parsing, transaction writer, masked views and secret-set
  capture for named scanner credentials and app associations.
- Build per-app bounded scanners. Keep both credential planes isolated, batch
  only compatible IDs, preserve shared counters and global report limits.
- Wire selection into report/discovery/direct results and reports. Preserve
  existing single-scan and bulk workflows with explicit legacy behavior; implement
  named submission selection and bound resume state without credential fallback.

Tests: two apps/keys with server-enforced ownership; unbound/ambiguous/mismatched
associations; exact app-name buckets; rotation; scanner-only tenant; token versus
key; wrong-plane leakage; safe config input; every read path remains read-only.
Also test unbound strict report and binding remedy, private rollback snapshot,
old binary's new-table refusal, old bulk state and named-state missing/rebound refs.
Cover both flat secrets configured, explicit --use key/token and absent-kind refusal.

### A2 — 8.1 evidence recovery and precise diagnostics

- Preserve typed session/action detector breakdowns as positive management
  evidence, retaining source/unit/direction/action uncertainty.
- Prioritize known-positive sessions with fairness across app buckets. Keep
  aggregate violated-session metrics visible beside unresolved scan outcomes.
- Add sanitized requested/returned/usable/pending/rejected/missing and attempt
  diagnostics per app and source. Preserve unknown errors without invented causes.

Tests: malicious+allow; mixed detector actions; management violation with no
scanner credential; aggregate-only violations; empty results versus HTTP failure;
page budget exhaustion; late app fairness; missing snippets; global two-send cap.

### A3 — 8.1 release-candidate qualification

- Run required full checks and offline saved-evidence compatibility. Build Linux
  amd64/arm64 musl with cargo zigbuild on enterprise and macOS arm64 on an
  available named Mac host. Execute arm64-musl loopback/offline smoke tests on
  a native Linux arm64 host or a documented emulator, not just a cross-compile.
- Verify packaged versions, hashes, extraction, private bundles and loopback
  results/report workflows. Update docs, upgrade notes and RC validation evidence.
- Supply a narrowly budgeted user-run qualification recipe comparing a known
  scan and its associated app credential. Wrong-key/rotation probes are optional,
  explicit and bounded; do not claim original-key versus app scope is resolved
  by synthetic fixtures. Only the user runs real-service qualification.

Gate: independent >=9 review and user-run qualification remain distinct. No
signing/publishing workflow changes or automatic publication.

### B0 — 8.2 history schema, dependency and contract

- Freeze command/flag grammar and applicability for sync/history/report,
  import/export/prune, live selectors and offline conflicts. Set exact normalized
  cloud identity, filesystem path rules, source coverage semantics and exit codes;
  include legacy report flags and explicit --refresh/--with-examples behavior.
- Prototype bundled SQLite on Rust 1.89, both Linux musl architectures with Zig C
  compilation and macOS arm64; execute every shipped target natively or through
  the documented emulator. Specify schema, indexes, logical migration/backup
  format, DELETE journal safe open modes and contention/lease behavior.
- Freeze the receipt-promotion predicate, bounded result/report retry transitions,
  cloud/service cooldown, tolerant rolling traversal, boundary uncertainty,
  traversed-as-of states and falling-behind criteria before ingestion code.
- Specify scanner-only cloud namespace derivation from configured/default
  management environment plus pinned TSG without acquiring management credentials;
  refuse ambiguous custom environments and warn once if retention is ineligible.
- Freeze the two-second read deadline, 250 ms retention busy limit, maintenance
  gate and VACUUM INTO private destination/swap/recovery protocol before storage.
- Prepare synthetic duplicate/conflict/partial-window fixtures and benchmark
  generator; no production/customer evidence in fixtures.

Gate: compiled cross-platform feasibility and documented invariants precede
binding library adoption and automatic persistence.

### B1 — SQLite durability and repository layer

- Implement safe archive initialization, versioned transactional migration,
  parameterized storage and read-only queries; integrate private sidecar handling.
- Implement idempotent observations, source windows, profiles and coverage with
  exact identities, meaningful conflict preservation and rebuildable summaries.
- Implement status/coverage inspection and explicit maintenance/error behavior.

Tests: interrupts/crashes at transaction boundaries, disk-full/write errors,
read-only missing/corrupt/newer databases, failed migrations, rollback, concurrent
readers/writers, symlink/ownership/mode refusals, no files created by readers,
no reader database/journal mutations, hot-journal refusal, identity isolation,
alias continuity and no secrets in main/journal/temp outputs. Reject WAL archives
without silently changing them; a future journal-mode migration is separate work.
Exercise long-reader retention skips, read deadline enforcement, maintenance gate
contention, compaction races/interruption, no live journal after prune and marker
absence throughout the whole live database.

### B2 — Automatic scan-result retention

- Retain already-received scan/bulk/resume outcomes through one normalized writer.
  Persist available progress before interruption without changing remote scans.
- Preserve invocation results if local archive writes fail; emit safe warnings.
  Keep existing bulk polling state separate from the evidence archive.
- Implement --no-history and one persistence warning per invocation. Retain no
  full submitted text or masked prompt/response echoes; B2 stores metadata only.
  Sanitized examples enter only from the explicit bounded report-evidence path.

Tests: repeat resume, partial batches, pending/failed/unknown outcomes, warning
stream/quiet semantics, no extra HTTP calls or scan resubmissions, missing app
identity quarantine and credential rotation. Do not archive submitted payloads.
Test a retained receipt subsequently discovered by sync: deterministic supported
promotion, conflicting/multi-bucket refusal and no double-counted population.

### B3 — Resumable sync and explicit enrichment

- Implement recent bootstrap, stable work descriptions, incomplete-work revisit,
  overlap/dedup, fair budgets and separate metadata/detail progress.
- Add supported backfill controls and honest unavailable-window reporting.
- Add opt-in examples with persistent selection, cross-app limits, 429 cooldown
  and bounded retries across invocations. Ordinary sync never calls reports.
- Implement fail-fast per-archive sync lease, shifting-pagination partial runs,
  retraversal instead of saved offsets, traversed-as-of boundaries and backlog
  state. Bound result-enrichment revisits after credential rejection/rotation too.

Tests: pagination changes, late events, interrupted runs, stale cursors, expired
upstream history, no-credential apps, alternate app-name buckets, overlapping
collections, repeated 429/missing report, clock changes and request accounting.
Test scheduled sync racing report --refresh: only one owns the network lease.

### B4 — Portable history and lifecycle

- Implement existing-bundle import with explicit cloud identity and immutable
  original preservation; implement versioned archive export/import and validation.
- Add previewed selective prune, statistics, backup/maintenance guidance and
  observation/provenance handling after detail removal.

Tests: Linux/macOS interchange, repeated imports, identity mismatch, malicious
  files/digests, redaction/identity collisions, older schemas, conflicts, secret
  suppression, no-clobber private outputs, requested-only deletion, removed
  content absent from live free pages, logical exports and compaction limits.

### B5 — Historical reports and interactive HTML

- Switch default report input to SQLite; implement --refresh and historical
  ranges while keeping from-evidence offline. Reuse normalized evidence, rule
  versioning, review binding and private no-clobber bundle publication.
- Add scoped trends, date/app/detector/verdict/action filters, freshness/coverage
  indicators, observed profile change history and the profile/service matrix.
- Enforce summary/detail population consistency, bounded exports and explicit
  truncation. Preserve native keyboard/print behavior and offline CSP isolation.

Tests: UTC boundaries, missing/empty/partial archives, stale snapshots, scan
versus session units, overlapping aggregate windows, unknown/conflict display,
malicious+allow, exact filtered counts, capped detail behavior, offline network
denial, HTML injection, mobile/keyboard/print, evidence/review reproducibility.

### B6 — Scale, schedules and 8.2 release qualification

- Benchmark one million synthetic distinct scans across multiple apps and years,
  repeated observations and sparse/conflicting coverage. Measure ingest, query,
  database size, peak memory, exported HTML size and 30/90-day report timings.
  Set reproducible numerical pass thresholds on documented test hardware before
  accepting this phase; a generated row count alone is not a performance result.
  Include the shipped amd64-musl build in benchmarks, not only glibc debug/release.
  Measure peak memory for reports and compaction, read transaction deadlines and
  retention busy outcomes under concurrent long reads.
- Provide optional hourly metadata-only systemd and launchd examples, explicit
  configuration/data paths, safe logs, overlap prevention and recovery guidance.
- Run required checks, loopback end-to-end workflows and native/archive tests on
  enterprise, Linux arm64 natively or emulated, and a Mac. Update command/reference
  docs, DESIGN, COMPATIBILITY,
  testing/operations/upgrade notes and release notes. Produce test binaries and
  manifests using the existing pipeline conventions.

Gate: independent >=9 phase review, all required checks, validated binaries,
documented remaining limitations and separate user-run live qualification.

## Required validation and implementation discipline

For each phase use focused meaningful tests; before completing a release-sized
change run:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
AIRS_REQUIRE_PARITY=1 cargo test --workspace
```

Use Node 24 and the exact frozen reference. No parity skips, SDK checkout edits,
v7 tag deletion or real API calls from agent-run tests. Run all network tests on
the loopback harness and keep path/data/config changes inside its sandbox.
Extend its guards and snapshots for SQLite files and scheduler examples without
weakening existing protections. Document and test every observable difference
in DESIGN/COMPATIBILITY, including paired dNN parity tests where required.

Keep independent review records with the reviewed revision, score, findings,
corrections and rescore. Never replace an unsuccessful review with a claimed pass.
No implementation phase begins with an unresolved prerequisite. Existing work
and user bundles remain intact; never package private bundles, credentials or
customer content in public fixtures or development archives.
