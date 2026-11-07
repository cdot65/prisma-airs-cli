# Runtime history storage contract — 8.2, phase B0

Status: B0 contract independently accepted9/10; later phases implement it. This
is not an assertion that history commands already exist. RC4 remains the qualified 8.1 candidate. No runtime SQLite dependency
or automatic persistence is adopted until B0's isolated prototype passes on all
three shipped targets and independent review reaches 9/10. SDK remains exactly 0.2.4.

## Mandatory Claude C1 resolution before database code

Ordinary report/coverage/status reads use one coherent SQLite read transaction,
including all related summary/detail/eligibility queries, with a monotonic 2-second
budget including lock wait. They do not use chunked mutable-row cutoffs. Capture
bounded data, release locks/transaction, then render. A timeout yields an operational
error and no report publication; it never publishes a mixed/partial snapshot as
complete. Historical completeness and deliberate display truncation remain distinct.

Whole-archive export, full statistics and prune preview are explicit maintenance
operations. They acquire the existing writer lease, then the exclusive maintenance
gate, then SQLite, in that order. They have a separate 120-second operation budget
(configurable 1–3600), read bounded chunks, and release each chunk transaction promptly.
The exclusive gate prevents every cooperating retention/sync/import writer and
ordinary reader from crossing those chunks. There is no mutable cutoff shortcut.
They create no file merely to read an existing archive. Missing gate/lease files
cause refusal, never repair by readers. Large prune and compaction use this same
exclusive protocol; automatic retention skips rather than waiting indefinitely.

The TOML transaction lock is never held while taking archive locks. Network work
never happens inside a SQLite transaction or while holding the maintenance gate.
Sync holds the per-archive network/writer lease for its invocation, before its first
API request. Retention does not acquire that network lease: it takes the exclusive
gate for a bounded write batch, interleaving between sync commits. Ordinary readers
take the shared gate. This B1 refinement serializes every mutating SQLite open
before its sidecar checks, preventing a cooperating writer from crashing between
a peer's precheck and implicit SQLite recovery. No gate is held across network.
Maintenance takes the network lease and exclusive gate, so it excludes both paths.

These choices resolve C1, lock ordering, missing-gate behavior and coherent report
queries. They are prerequisites to the prototype and repository integration.

## Dependency and platform gate

Candidate: exact rusqlite 0.41.1, default features disabled, features bundled/hooks/
limits. B1 adds the safe serialize wrapper for memory-built initial databases
written through owned descriptors; this feature addition has its own B1 platform
qualification. Its released manifest declares Rust 1.89.0 and its bundled SQLite is compiled
for the target, rather than linked to an end user's SQLite installation. No SQLCipher,
load-extension, virtual-table, serde JSON integration or raw SDK serialization feature
is enabled. Project code continues to forbid unsafe; dependency FFI stays inside the
binding. The prototype records actual SQLite version/source ID, compile options,
Rust/compiler/target and feature configuration. Final adoption requires actual Rust
1.89 execution/type checking, amd64/arm64 musl execution, and native macOS arm64.
See the [released binding manifest](https://raw.githubusercontent.com/rusqlite/rusqlite/v0.41.1/Cargo.toml).

Connections use DELETE journaling, synchronous FULL, foreign_keys ON, secure_delete
ON (never FAST), temp_store MEMORY, mmap_size 0 and an 8 MiB page-cache target. Set
connection/SQL length, expression, attached-database and variable limits. Disable
trusted schema and extension loading. No user-supplied SQL or attached databases.
Ordinary connections set the attachment limit to zero. Only the fixed maintenance
VACUUM INTO statement permits one internal destination attachment, then restores
the limit; no ATTACH command is exposed. Set a 256 MiB SQLite allocator limit once
per CLI process, before opening archives; this is not a total-process RSS limit.
Record measured report/compaction RSS in qualification.
Enabling binding callback APIs does not authorize WAL files or WAL journal mode.
[SQLite journal semantics](https://sqlite.org/pragma.html#pragma_journal_mode)
explain why DELETE commit removes the rollback journal; this is verified, not assumed.

## Cloud identity and data root

Configuration remains exclusively in tenant TOML. No credentials/endpoints are read
from environment or history. A history reference never carries a secret, fragment,
auth_code, api_key_last8 or secret-derived hash. All configured credentials and OAuth
leases join the invocation's existing privacy registry before retention/output.

The environment descriptor is the canonical JSON tuple of admitted management,
dashboard and token endpoints, in that order. Canonicalize with the existing URL
admission and strip trailing slashes; preserve meaningful paths, ports and schemes.
No name, region or customer-app-ID inference. Known public defaults are exactly:

- management/dashboard: https://api.apps.paloaltonetworks.com/aisec
- token: https://auth.apps.paloaltonetworks.com/oauth2/access_token
- public default scanner: https://service.api.aisecurity.paloaltonetworks.com

For a custom environment all three management-plane endpoints must be explicitly
configured and admitted, even for a scanner-only tenant; resolving identity obtains
no OAuth lease. Missing custom components make retention ineligible, with one safe
warning, while the scan outcome is preserved. Default-environment automatic receipt
retention also requires that the selected scanner endpoint is the known default
scanner endpoint; an arbitrary custom scanner URL cannot inherit a guessed cloud.
Management sync derives its namespace from its explicitly selected management
context. Scanner-result attribution still requires the exact operator binding.

Environment ID is env1:<SHA256 of the public endpoint tuple>. Cloud ID is SHA256
of canonical JSON [1, exact pinned TSG, environment ID]. TSG must be 1–32 ASCII digits.
Equal normalized endpoint tuples and TSGs share history across nicknames and secret
rotations. Different tuples/TSGs never merge. Hashes identify public namespaces,
not credentials. Archive meta stores the descriptor and pin and must match on open.
Unknown custom endpoints are never aliased by hostname resemblance or display names.

Data-root precedence: explicit --history-dir, then $XDG_DATA_HOME/airs-cli/history,
then $HOME/.local/share/airs-cli/history. Flags are absolute/nonempty and validated
before tenant reads; invalid explicit paths exit 2. Invalid environment path controls
exit 1. There is no dataDir TOML setting or scanner configuration sidecar. One cloud
uses ROOT/<cloud-id>/archive.sqlite3, writer.lock and maintenance.lock. Readers do
not initialize anything. Missing archives are unavailable, never zero activity.

Controlled directories must be owned by the current user, real directories and
0700 or stricter. Files must be regular, current-user-owned, 0600 or stricter, with
no symlink/hard-link replacement accepted. Create privately from the outset;
never silently chmod a loose existing archive. Normalize existing parent aliases
(e.g. macOS /var) before the controlled root; validate ancestors against hostile
rename: owner current user/root, no group/other write except root-owned sticky
system temporary directories. Controlled roots/cloud children are never symlinks.
Use checked descriptors/identity comparisons around publication, locking and opens.
Readers may lock existing gate descriptors but may not create/modify lock files.

Live archives are supported only on local filesystems. Refuse identified NFS/SMB
mounts before database access; failure to determine filesystem type is an error.
Document unsupported network/cloud-sync directories and use logical export/import
for transfer. Filesystem classification cannot detect an arbitrary sync client.

## Command grammar and applicability

New scalar flags reject repetition before input/store access. Existing repeatable
application IDs retain their semantics. All time ranges are UTC half-open [since,
until); require explicit Z timestamps accepted by the existing ISO parser and
since<until. Invalid syntax/conflicts exit 2 before tenant or archive access.
Explicit since/until are paired: supplying only one is a usage error. Sync uses
24 hours when neither is given; backfill requires both. Supplying interval/unit
with either absolute bound is a usage error. Unknown/live-only selectors do not
silently apply to history reads: sync/report credential routing remains per exact
application association, with no global --scanner override. The existing repeatable
--app-id filter applies to both sync and historical report reads.

Existing report destination/title/strict/review behavior remains, except review
continues to require --from-evidence. Local --max-events is a 1–10000 display-row
cap, not a population-count cap; summaries/interactive filters must use the full
eligible cohort or explicitly fail on unsupported scope. With refresh it also
bounds that invocation's live collection. Explicit --timeout/--concurrency/
--max-requests/--max-pages are live controls requiring --refresh. Local ordinary
read time remains fixed at two seconds. --interval/--unit accept existing bounds
and become local historical selectors; sync accepts absolute bounds only.

- runtime sync [--since UTC --until UTC --backfill] [--app-id ID...]
  [--with-examples] [--max-reports 0..10] [--history-dir PATH]
  plus the existing timeout/concurrency/max-requests/max-pages/max-events controls.
- runtime report defaults to archived data. It accepts --since/--until or existing
  --interval/--unit for local ranges, not both; default 24 hours. --refresh performs
  sync first, then renders local data. --with-examples requires --refresh.
  --max-reports requires --refresh --with-examples when explicitly supplied.
  Historical --app-id filters apply to reads; live budgets require --refresh.
- --from-evidence remains entirely offline and ignores selected tenant, as before;
  it conflicts with history-dir/refresh/with-examples and other explicitly live
  or archive-range controls. Existing review/title/output/bundle validation stays.
- runtime scan, bulk-scan and resume-poll gain --no-history and --history-dir.
  They conflict with each other when both explicitly supplied. Without opt-out,
  only metadata already received from those operations is eligible for retention.
  Direct scan-logs/dashboard commands retain their live semantics in 8.2.
- runtime history status [--history-dir PATH] is an ordinary non-writing inspection.
  It may successfully report archive_present=false with null/unknown counts.
- runtime history coverage [--since UTC --until UTC] [--app-id ID...]
  [--history-dir PATH] is an ordinary bounded read, default 24 hours.
- runtime history stats [--timeout 1..3600] [--history-dir PATH] is explicit full
  maintenance inspection, default 120 seconds, with no database/data creation.
- runtime history export --file NEW_PATH [--timeout 1..3600] [--history-dir PATH]
  is full maintenance and writes a private no-clobber logical export only.
- runtime history import --file PATH --expect-tsg ID --expect-environment ENV_ID
  [--timeout 1..3600] [--history-dir PATH] validates logical export before mutation.
- runtime history import-bundle --file EVIDENCE_JSON --expect-tsg ID
  --expect-environment ENV_ID [--history-dir PATH] imports validated legacy evidence;
  explicit binding is mandatory because legacy bundles lack a canonical cloud ID.
- runtime history prune --before UTC [--app-id ID...] [--examples-only] [--force]
  [--timeout 1..3600] [--history-dir PATH] previews by default; --force explicitly
  performs the same deterministic selection under the exclusive gate.
- runtime history compact --force [--timeout 1..3600] [--history-dir PATH], default
 600 seconds, uses the verified-copy/swap procedure below.
- runtime history recover --force [--timeout 1..3600] [--history-dir PATH] is the only
  explicit path for stale journal recovery; it never silently rebuilds corrupt data.
- runtime history retry --source results|reports --force [--history-dir PATH]
  explicitly resets bounded deferred work, preserving attempt/authorization history.

Local history leaves build no API client. stdout is data only; stderr is status/
warnings/errors. Quiet suppresses status, not persistence/durability warnings.
Exit 0 success; 1 operational/incomplete strict sync/report, missing/corrupt/newer/
unsafe archive, lease contention, timeout; 2 usage; 130/143 interrupted. Existing
optional example gaps do not independently fail strict primary metadata collection.
A refresh failure can still render available history with explicit stale/partial
provenance; it exits operationally after publishing those qualified artifacts.

## Records, eligibility and receipt promotion

Keep full events keyed by (app_id, exact app_name, session_id, scan_UUID,
explicit sub_request) within the cloud. Event time, first/last observation time
and collection/run time are separate. Raw scan UUID cardinality and full event
observations are distinct units; counting UUID strings does not join identities.
Unknown event time is retained but excluded from date-window totals.

Separate immutable normalized facts, mutable resolved projections, receipt facts,
examples, observed profiles, aggregates, runs, per-source coverage, sync work and
cooldowns. SQL is parameterized. Meaningful fact identity excludes run UUID and
observation time so repeated identical responses do not create new facts. Run/fact
links retain collection provenance. Unknown fields can be resolved by added facts;
contradictions are preserved and resolve to explicit conflict/unknown, never last-wins.
Raw SDK extensions, full payloads and submitted prompt/response echoes are absent.

A receipt key is stable per bulk job/item or synchronous invocation/item. Preserve
all meaningful receipt variants. Its identity fields can be absent, never defaulted.
It is excluded from attributed event/scan counts until deterministic promotion:

1. Same cloud and exact canonical scan UUID; every supplied session/subrequest/
   report identity must agree. Any explicitly selected app ID must agree.
2. No conflicting known receipt fields or incompatible endpoint/binding provenance.
3. Exactly one fully supported full-event identity matches all supplied fields.
   A scan-ID-only coincidence with no app/session/subrequest support is insufficient.
4. Attach provenance to that event and remove provisional eligibility atomically.
   Both ingestion orders produce the same result. A later competing candidate or
   conflicting fact invalidates a previous promotion and recomputes projections.

Unpromoted/ambiguous/conflicting receipt counts remain separate and labeled.
The schema enforces that a promoted receipt has a non-null event reference and
every other state has none. Event deletion is restricted until the same prune
transaction explicitly demotes/detaches affected receipts and recomputes their
eligibility. Pruning a full event cannot silently leave a promoted orphan; retained
receipt metadata remains a separate partial population. Reconciliation can later
promote it again only using the complete predicate above.
Profile observations record observed configuration, not asserted effective-change
instants. Historical posture uses the latest supported observation at/before the
requested end; absent observation is unknown, never today's policy retroactively.
Aggregate session/token/API totals retain source, population, scale and exact or
explicitly uncertain window. Never sum overlapping rolling aggregates as traffic.

## Sync windows, work and retries

Bootstrap is 24 hours. Ordinary sync does not silently backfill before its bootstrap
boundary; re-query overlapping rolling windows and deduplicate. Persistent work
uses stable app/name/session/scan descriptors, not saved server offsets/cursors.
Each resumption re-queries from a supported head and validates totals/duplicates.
Partial pagination stays partial; shifting datasets and late observations remain
explicit. Work that cannot catch up within request/page/event limits is falling
behind. An offline gap outside the default recent window is recorded, not erased.

--backfill requires since/until. The SDK has no absolute historical selector:
query a supported rolling interval covering the requested lower bound and filter
locally. Never claim the server honored absolute bounds. Requests older than the
87600-hour CLI bound are usage errors; server retention/missing history is unknown,
not a not-found conclusion. Keep requested bounds, actual query and traversal times.
Coverage is traversed-as-of with boundary/late-arrival uncertainty, not a sealed
provider watermark or proof of all historical traffic.

Ordinary sync/report refresh makes zero Reports API calls unless --with-examples
is explicit. Cached usable examples need no request. Opt-in selection is strategic,
five IDs default/ten maximum and two actual serial sends across all credentials,
zero SDK retries, stop globally after 429. Selection and attempt provenance persist.

Results and optional reports have at most 3 eligible attempts per work identity/
binding revision, spaced at least 1 minute then 15 minutes; exhausted work is deferred
until an explicit retry reset or changed binding revision. Unknown/missing alone
never proves authorization failure. Attributable 401/403 makes that app/revision
ineligible immediately. Removed/rebound associations remain ineligible. A same-app/
endpoint revision change permits one currently configured credential only, with
unqualified historical-access provenance, never a fallback to other keys.

Each actual attempt has an append-preserving work-attempt record with run,
transport ID, non-secret binding revision, retry epoch/ordinal, start/end, safe
status classification and provenance. One multi-ID transport can serve several
work attempts without being counted as several HTTP sends. Insert a `started`
record before sending; finish that record once, and preserve an interrupted/
uncertain send on restart rather than silently replaying it for free. Attempt
admission reserves its ordinal conservatively; a proven no-send admission finishes
as `not_sent`, releases that ordinal and cannot count as an actual send or eligible
attempt. An uncertain interrupted admission retains the ordinal, preventing a
free replay. Runtime SDK budgets
still enforce actual sends independently. Retry resets, binding changes,
authorization deferrals, cooldowns and completion transitions are durable separate
records. Reset increments the retry epoch in the same transaction as its transition;
it does not delete old attempts or authorization evidence. The mutable sync-work
row is a rebuildable scheduling projection, not the only attempt history.
These ordinal/revision rules apply only to results/reports. Session work has zero
scanner attempts, no scanner binding revision and no retry epoch; management
traversal can require more than three pages/resumptions. Its individual reads are
recorded separately with run/source/application, actual send, safe status/outcome
and bounded request-descriptor provenance. Invocation request/page/event/time
budgets still bound discovery. The schema's composite work/source reference
prevents a scanner-attempt record from being attached to session work.

A 429 stops that service's remaining invocation work and persists a cloud/service
cooldown across all named credentials: use a valid bounded Retry-After when present,
otherwise 15 minutes, maximum 24 hours. Both positive delta-seconds and future
HTTP-date headers are usable; round dates up to avoid an early retry. Zero, past
dates and malformed headers use the conservative fallback. Later ordinary metadata sync still makes no
report calls. Failed/budget-denied work with no actual transport send does not
consume a work attempt. Failed/timed-out scan outcomes never become allow. Within
an invocation use monotonic deadlines; detected backward wall-clock changes defer
cooldown/retry work with explicit uncertainty. Do not claim to detect arbitrary
cross-process forward clock changes; hard request limits do not depend on the clock.

## Write bounds, filesystem recovery and maintenance

Automatic retention has a 250 ms total gate/SQLite contention budget per batch, no
spool and no hidden retry. Skip/count the unarchived results and emit at most one
safe persistence warning per invocation, even under quiet. Scan/bulk outcomes and
remote submission behavior are unchanged by local persistence failure. Writes
batch at most 200 records/256 KiB normalized data and target <=100 ms lock hold; split
larger sync/import work. Disable cache spill for bounded writes so a busy commit
can roll back without introducing a long writer hold. Explicit operations have
separate deadlines; transactions never span network, output or user interaction.
Capture one monotonic contention deadline before the first gate attempt and pass
that same deadline to SQLite's busy/progress callbacks; never restart 250 ms after
the gate. The prototype forces about 100 ms of gate contention followed by a
blocked SQLite writer, and checks the combined elapsed time against 250 ms with
at most 50 ms of observed OS scheduling allowance. That allowance is a test bound,
not extra configured wait time or a promise of hard real-time filesystem I/O.

Read-only opens validate schema/cloud/journal mode without migrations or recovery.
Reject WAL/SHM archives and unsafe journal/temp paths, never switch journal mode.
Wait only within the read budget for an active short writer; an unfinished journal
requires explicit recover. Recovery holds writer lease/exclusive gate, validates
all private owned files, refuses unexpected/super-journal paths, preserves recovery
material on failure and never deletes/recreates an unreadable database. Migrations
are explicit writer transactions with version checks and verified private recovery
backups. Refuse newer schemas and leave original bytes unchanged on failure.

Prune uses secure_delete=ON and DELETE journal mode, with verified absence of a live
journal on success. Test marker absence throughout the entire live database,
including cell slack/freelist. Logical exports contain retained normalized records
only. Existing external backups/filesystem snapshots/SSD blocks are outside this
logical deletion promise; no forensic-erasure claim.

Compaction requires >=2x current database bytes of free space, existing private
root and no active ordinary readers. Pre-create an empty private 0600 destination
with no-clobber; VACUUM INTO that exact path under the exclusive protocol. The
compaction connection also uses MEMORY temp storage with a measured heap/RSS bound.
Validate copy schema/cloud/integrity, fsync file and directory explicitly, recheck
source/destination identities, atomically replace and fsync the directory. Failed/
interrupted copies cannot become live; clean only owned staging files. A failed
post-swap durability check is reported as uncertain, not silently rolled back over
new committed data. No process-global temp-directory/environment mutation.
[SQLite's VACUUM INTO contract](https://sqlite.org/lang_vacuum.html) permits an
existing empty destination and describes its snapshot and interruption semantics;
our explicit validation/fsync/swap remains required independently.

## Schema/prototype and acceptance evidence

### Versioned logical transfer and migration backups

Logical transfer version 1 is UTF-8 JSON Lines: one strict header, typed record
lines, then one footer. The header identifies format `airs-runtime-history`,
format/schema/analysis/redaction versions and the canonical public cloud descriptor.
The footer contains per-record-type counts and SHA256 over the exact header and
record bytes, including newlines, excluding the footer. Reject unknown versions,
record kinds/fields, duplicate primary keys, missing/duplicate footer, trailing
content, invalid UTF-8, malformed relations and mismatched digest/counts. Each line
is at most 1 MiB; total input is at most 16 GiB. Decode bounded records, never a
whole archive in memory. Exports contain normalized allowlisted fields only and
never SQL, database pages, tenant configuration, secret references derived from
secret bytes, full prompts/responses or raw SDK extension data. No physical SQLite
file can be imported as a logical transfer.

Import requires explicit expected TSG/environment and checks both against the
header and selected local context. Before destination mutation, validate into a
private no-clobber staged logical copy, validate its entire digest/relations, fsync
it and ensure space for staging plus bounded destination writes. Import only that
validated owned copy in bounded idempotent transactions. A failure after committed
batches leaves a labeled partial import run with its verified transfer identity;
reimport safely resumes/deduplicates. Do not claim all-or-nothing for the entire
archive. Clean only owned staging files; a changed external input cannot change
already validated records. Existing archive identity never changes during import.

Schema migration is an explicit writer operation under the network/writer lease
and exclusive maintenance gate. Before the first migration write, create a
private no-clobber consistent SQLite backup with the same verified-copy protocol
as compaction, retaining a versioned manifest (old schema, cloud ID, SQLite version,
SHA256, byte count and timestamp). Validate integrity/cloud/schema and fsync the
backup and directory before starting the transactional migration. Commit version
and content together, verify afterwards, and retain the backup until explicit
operator deletion. Roll back a failed transaction; never silently replace a
damaged live file from a backup. An unsupported newer schema is refused before
any backup/write. Schema 1 is the initial version, not a claim of a legacy SQLite
schema or migration that does not exist.

B1 starts at schema1 with versioned admission/refusal and no supported migration.
The independent B1 reviewer accepted avoiding an unused generic migration engine:
there is no published predecessor. A real schema2 change must add its concrete
transactional migration and the backup/failure gates above before shipping.

The B0 prototype's version 1 schema is the reviewed basis for B1, with explicit keys,
foreign keys and covering indexes for cloud/app/time/scan/eligibility queries.
Prototype data is synthetic only. Produce duplicate/conflict/partial-window fixtures
and a reproducible million-scan multi-year generator. Record 30/90-day query times,
ingest rate, database size, page-cache/temp choices and peak memory; B6 repeats
end-to-end reports/compaction/retention-contention on all shipped targets. A 30/90-day
report at the 1M-scan dataset must complete its coherent read within 2 seconds on the
documented hardware; timeout-and-narrow does not substitute for this gate.

B0 must demonstrate bundled compilation/execution, private journal behavior,
read-only non-writing behavior, deadline interruption, secure-delete marker removal,
VACUUM INTO private-copy validity and initial index feasibility. Runtime ingestion,
automatic retention and CLI commands remain B1–B5 work until those phases pass.


### B3 implementation refinements (unpublished development schema)

The initial schema1 adds a rebuildable event_reports ownership relation populated
from full facts and currently promoted receipts. Indexes support global ownership,
authorization, promoted-receipt bounded rehydration and management clock checks without scanning all archived JSON per
candidate. Draft development schemas with a different exact signature are refused
unchanged; this is not a claim of a migration path or a published schema upgrade.
The independently frozen B0 benchmark schema remains unchanged.

A bounded combined example pool considers current and eligible retained candidates.
Cached usable examples cover their features without a request; reserve at most one
selection slot for unfinished old work, then choose uncovered features. A one-ID
budget alternates old/new preference when both are available. Metadata candidate
views reserve at most half the event budget; a one-event budget alternates retained
and recent preference by invocation. Candidate bytes, global ID/send budgets and
per-source attempt rules remain binding. No old individual-ID lookup establishes
historical traversal coverage.

Internal native snapshots may retain ambiguous report context, while published
version1 evidence validation remains strict. Only supported unique ownership is
eligible for a Reports API request. A local oversized-view deferral records the
public revision evaluated without claiming a send; explicit reset can reconsider it.

Report-policy eligibility (malicious projection with known event time) is filtered
before the due candidate limit, so unsupported old examples cannot strand later
eligible work. Proven no-send and observed authorization/rate outcomes are committed
through the bounded finalizer before cancellation-sensitive snapshots. Fixed safe
management clock/cooldown admission notes survive in progress/provenance. Primary
availability excludes optional telemetry and derived archive metadata; an empty
inventory establishes health only when its primary traversal is Complete.

Privacy-altered or prewithheld supplied identity/report constraints remain distinct
from missing-at-source values. Immutable normalized facts and receipt bindings carry
only a non-sensitive identity_withheld boolean. Any such fact prevents promotion;
a later flagged fact demotes an earlier promotion and recomputes the full-event
projection. The initial unpublished schema requires an explicit boolean receipt
binding marker; earlier draft signatures are refused unchanged rather than guessed
safe. No removed value or removed-value hash supplies a durable join key.
