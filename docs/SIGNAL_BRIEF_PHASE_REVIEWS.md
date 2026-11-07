# Signal Brief independent phase reviews

Scores concern the completed phase scope, not completion of the whole feature.
Every phase requires an independent score of at least 9/10 before advancement.
Real-service qualification, scans, pushes and releases are outside these reviews.

## Phase 0 — SDK feasibility and proposed contracts

Reviewer: independent Codex instance `/root/phase0_review`, 2026-11-02.
Reviewed `tests/signal_sdk.rs`, `docs/research/rs-sdk-signal-brief.md`, approved
implementation plan, contributor contract and relevant pinned 0.2.4 SDK source.

Initial score: **8/10 — revise before advancing**.

The capability matrix and proposed-coverage ledger correctly distinguish the pinned
crate from a newer checkout, preserve both action levels and exact application-name
buckets, and describe synthetic compatibility separately from live semantic
qualification. The synthetic chain obtains a scan result's report ID and retrieves
a typed PI snippet plus benign DLP context without submitting a scan. No public CLI
behavior changes yet require a DESIGN divergence row.

One material finding:

- The SDK scanner in the new integration test uses its default ReqwestTransport,
  whose pinned implementation honors inherited environment proxies. Unlike the
  CLI's direct loopback transport, this can send fixture requests to a non-loopback
  proxy. Inject a direct, redirect-refusing, no-proxy transport, or reuse the
  existing direct transport from an internal test. Do not solve it by mutating
  process-global environment in the parallel test runner.

Small proof improvement: consume the dashboard-returned scan ID in the scanner
query, and compare its sub-request identity with the scan-result and report request
identities, explicitly keeping this a synthetic association rather than a claim of
real-service field equivalence.

Checks inspected: root's focused test passed; initial Clippy failed only on
`cloned_ref_to_slice_refs`, which root reported fixing before this review. Final
focused test/format/lint results and re-score will be recorded after corrections.

### Phase 0 final re-review

Final score: **9/10 — phase gate passed**. No unresolved material Phase 0 findings.

The root injected a dedicated transport that asserts the destination is the literal
loopback host `127.0.0.1`, disables proxies and refuses redirects. It consumes the
dashboard-returned scan ID for results lookup and checks the modeled result/report
request IDs against the session sub-request. The fixed Clippy slice construction
also passes. These assertions strengthen the synthetic proof without promoting it
to real-service identity qualification.

Independent verification performed by this reviewer:

- `cargo fmt --all --check`: passed.
- `cargo clippy --test signal_sdk -- -D warnings`: passed.
- `cargo test --test signal_sdk` with HTTP_PROXY, HTTPS_PROXY and ALL_PROXY pointing
  at an unusable loopback proxy and empty NO_PROXY: passed, 1 test, 0 failures.

The transport test contacted only the fixture. No production evidence, credentials,
SDK checkout changes or real service calls were used. Later phases still need
multi-request ambiguity tests, richer snippet presence cases, masking, live
collection limits and command behavior coverage; this score covers the minimal
pinned-SDK compatibility chain and proposed contract ledger only.

## Phase 1 — versioned evidence, privacy and offline foundation

Reviewer: independent Codex instance `/root/phase1_review`, 2026-11-02.
Scope reviewed: `src/report/signal/{mod,evidence,privacy}.rs`, the offline CLI
integration and sandbox path guard, synthetic fixture, unit/executable/parity tests,
D337 and DESIGN §18.14.1. The richer Phase 2 renderer and later live collector are
not required to exist at this phase gate. Existing default live output stays intact.

Initial score: **8/10 — revise before advancing**.

The duplicate-rejecting parser, immutable evidence digest check, typed source
references, explicit missing snippet states and offline-before-tenant dispatch are
sound foundations. Focused tests and root's all-target Clippy passed. However, an
independent forged-valid-digest case found a material privacy leak: a displayed
identity containing a credential could be projected before a later Authorization
string caused that credential to join SecretSet. The rendered HTML retained it.

A first two-destructive-pass correction removed the full-token case but still
leaked a token prefix if the first pass truncated that earlier field at 4096
characters. Discovery must populate the secret set using a disposable clone (or a
non-destructive discovery traversal), then project the untouched original document.
Require an executable regression covering both ordering and truncation boundaries.

Model hardening also needs direct rejection coverage for duplicate snippet
families, duplicate/phantom selected report IDs, and contradictory report-to-scan
ownership. Root has already strengthened several identity/source constraints and
window consistency during review. Final verification and score follow after fixes.

All reviewer probes used synthetic saved evidence in temporary private locations
with cleared environment and no tenant files. No real credentials, customer
samples, live API reads, scans or SDK modifications were involved.

### Phase 1 final re-review

Final score: **9/10 — phase gate passed**. No unresolved material Phase 1 findings.

The shared projection now discovers/registers credentials using a disposable clone,
then projects the original untruncated document. This resolves the ordering leak
and its truncation-boundary variant. The executable valid-digest regression places
a credential prefix across that boundary and confirms it does not reach HTML.
Titles also pass through the shared projection before escaping. DLP values are
registered before cross-family projection and fully withheld; explicit unavailable,
absent and empty states remain separate.

Schema validation now rejects duplicate/phantom selected report IDs, duplicate
snippet families, contradictory report-to-scan ownership, duplicate application/
profile identities, empty provenance, invalid snippet-state combinations and
inconsistent window/collection timing. Focused rejection cases cover these graph
and clock constraints. The digest remains an integrity check before privacy
projection, not an authenticity claim or reason to trust saved customer text.

Independent verification performed:

- `cargo fmt --all --check`: passed.
- `cargo test --lib report::signal`: passed, 6 tests.
- Parity-required `signal_offline`, `parity_signal`, `docs_consistency`: passed,
  respectively 3, 1 and 7 tests; no parity skip.
- Inspected root's final all-target Clippy result: passed with `-D warnings`.

This score covers the Phase 1 evidence foundation and intentionally simple offline
view. The later collector must still enforce real request/fan-out budgets and
explicit joins; the later renderer must retain privacy projection, honest counting
and all-surface escaping. Review packet import, bundle publication, strict-source
policy under live collection and richer accessibility remain their planned phases.
No live API calls or real tenant reads were performed during this review.

## Phase 2 — grouped analysis and standalone HTML

Reviewer: independent Codex instance `/root/phase2_review`, 2026-11-02.
Initial score: **8/10 — revise before advancing**.

Reviewed `src/report/signal/analyze.rs`, `html.rs`, renderer integration, the
Phase 2 contract/coverage updates, synthetic generated report and focused tests.
The structure, half-open window eligibility, explicit scan-versus-detector actions,
unknown/benign context, self-contained CSP and native evidence controls are sound.
The following material issues need correction before the Phase 2 gate passes.

1. **High — title-discovered credentials can leak from evidence.** At
   `html.rs:22–24`, projecting an Authorization-bearing title can add a credential to
   SecretSet after `signal::load` projected evidence. The renderer then prints
   previously projected fields without checking the final set. An independent
   forged-valid-digest probe put the same synthetic canary in scan_id and in
   `--title 'Authorization: Bearer …'`: exit 0, canary present in HTML. Complete
   title/evidence discovery before the final evidence projection, defensively at
   the rendering boundary, and add an executable cross-input regression.
2. **High — repeated source sightings inflate detector volumes.** At
   `analyze.rs:67–89`, rows are counted by event/vector positions. A valid snapshot
   can associate the same report_id/row and scan with two dashboard name buckets
   (the model intentionally preserves such source identities). The same detector
   observation then becomes two malicious rows and findings. An independent
   actual-CLI probe confirmed this. Deduplicate logical detector observations by
   report identity/row; retain all source associations and explicitly quarantine
   conflicting attribution rather than count it as another occurrence. Test
   repeated report rows across name buckets and sub-request ambiguity.
3. **Medium — ambiguous current protections can imply monitoring.** At
   `analyze.rs:114–123`, `.find` accepts the first matching detector/direction
   protection. A second matching protection with a conflicting action is ignored.
   Reject ambiguous configuration or mark it unknown/conflicting; never claim a
   monitoring interpretation from an arbitrary first match. Add a conflict case.
4. **Medium — detector volumes are missing as an aggregate view.** The approved
   supported sections include detector volumes, while the current brief provides
   one total card and per-application grouped findings. Add a detector/direction/
   enforcement table with explicit units and collected/sample population, deriving
   its counts from deduplicated observations, or visibly mark the view unavailable.
5. **Medium — only finding display is bounded.** At `html.rs:87–102`, profile rows
   and protections have no display cap. Valid 32 MiB evidence can contain a large
   number of small profile/protection rows with substantial HTML amplification.
   Add a documented cap/disclosure for that table and test actual boundary behavior,
   rather than only checking the small sample document's length.

Independent visual/behavior checks of the initial synthetic report:

- Chromium with JavaScript disabled and offline network routing: zero requests.
- Native summary activated with keyboard Enter: details opened.
- Mobile 390px viewport: no document-level horizontal overflow; CSP-approved inline
  styles applied (paper background verified).
- Print emulation: closed details' content visible with nonzero excerpt height.
- Extracted generated print PDF text contains the synthetic evidence excerpt.

These checks use only synthetic local data and do not qualify real-service
collection. Root's initial unit/offline/parity/docs checks and all-target Clippy
passed; the passing tests did not cover the independent probes above. Review score
and findings will be retained when the corrected phase is re-scored.

### Phase 2 final re-review

Final score: **9/10 — phase gate passed**. No unresolved material Phase 2 findings.

The renderer now discovers title credentials before any destructive input
projection, projects a defensive evidence clone with the completed SecretSet, and
finalizes the title after evidence-side discovery. A direct renderer regression
covers the reverse direction as well. Independent executable probes repeated the
original valid-digest title canary with a truncation-boundary prefix: exit 0, no
credential fragment in HTML. The cross-input privacy finding is resolved.

Analysis indexes logical detector observations by `(report_id, row)`. Conflicting
identity associations stay in saved evidence but are explicitly excluded from
malicious counts and grouped findings pending attribution. The independent repeated
name-bucket probe now reports zero malicious rows and a visible attribution gap,
rather than inventing another occurrence. Source event records and distinct scan
counts remain separate. Tests cover this quarantine and conflicting current
protection actions; ambiguous configuration no longer establishes monitoring.

The brief now includes sampled detector-volume rows with separate detector-row,
distinct-event and distinct-scan units. Profile display is capped at 250 rows, with
a disclosure, and actual boundary tests cover profile, event-row and excerpt caps.
A final 32 MiB HTML bound prevents publication of oversized output. The typed volume
accumulator also passes Clippy. Keyboard-focusable scroll wrappers and wrapping
styles improve narrow viewport access without removing semantic table captions.

Independent final verification:

- `cargo fmt --all --check`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --lib report::signal`: passed, 15 tests.
- Parity-required offline/parity/docs suites: passed, respectively 4, 1 and 7 tests;
  root's final title-boundary rerun also passed. No parity skip.
- Repeated both adversarial actual-CLI probes: title/truncation canary no longer
  leaks; duplicated report attribution is disclosed and excluded from counts.
- Earlier independent no-JavaScript, keyboard, mobile, CSP and print checks remain
  applicable; root regenerated the committed synthetic example through the final
  CLI renderer with an internally consistent synthetic 2026 window, and is checking
  its final visual output.

This gate covers the offline analysis/HTML phase. Live collection, sampling ledger
construction, bundle durability, review import and full final workspace validation
remain later-phase obligations. No real tenant, customer evidence or live API was
used in this review.

## Phase 3 independent review — initial

Reviewer: independent Codex instance `/root/phase3_review`, 2026-11-03 UTC.
Initial score: **8/10 — revisions required before the phase gate passes**.

Reviewed the new read-budget, shared lookup/collection and command adapters, bounded
client constructors, CLI parsing/output changes, schema additions, DLP projection,
phase plan and tests. Phases 0–2's prior scores do not qualify the new collector.
No production credentials, private customer evidence, live services or SDK edits
were used. An ignored local probe reused the loopback harness and real built CLI.

Material findings in the initial revision:

1. **High — failed/timed-out scan results can become benign/allow.**
   `collect.rs::enrich` copied category/action without validating wrapper status,
   error/timeout flags or errors. The independent actual-CLI probe supplied a
   COMPLETED wrapper containing `fixtures::scan_failed()` and observed exit 0,
   complete true, one row returned under `--filter benign`, action allow. Preserve
   raw observations if useful, but normalized failed/unknown outcomes must not enter
   benign/allow filters or monitoring recommendations. Require a known completed
   wrapper and successful result flags; test failed/error/timeout/unknown statuses.
2. **High — DLP matched text can leak through untrusted metadata.**
   The new excerpt-local span registry prevents short matches corrupting structural
   metadata, but only scrubs snippet excerpts. A report whose `source` repeats the
   same long value as its DLP snippet returned that known sensitive value verbatim;
   independent actual-CLI probe exit 0, leaked true. Project known DLP spans across
   untrusted displayed metadata too while keeping typed constants/correlation
   identities safe from short generic matches. Add metadata-echo coverage across
   direct and collected evidence outputs.
3. **Medium — valid large snippet arrays can abort shared collection.**
   `report_view` retained all excerpts and collector attached them to evidence;
   `privacy::project` capped string lengths but not vector length, while evidence
   validation requires <=20 excerpts. A valid report with 21 snippets would fail at
   seal instead of yielding bounded evidence. Apply one shared cap before validation,
   preserve total variant counts and disclose truncation; test collector and direct
   paths, including malformed/unknown snippet value shapes.
4. **Medium — a noncanonical UUID can poison a results batch.**
   Collector batching used permissive `Uuid::parse_str` while the pinned method only
   accepts 36-byte hyphenated UUIDs. A simple/URN UUID dashboard ID could enter a batch
   and prevent otherwise valid neighbors from being queried. Reuse the exact SDK
   grammar before batching and retain unsupported IDs separately as unresolved.
5. **Medium — application/selection fairness is lost before enrichment.**
   The initial session queue is flat inventory order; requeuing pages gives
   per-session fairness, not per-application fairness. Forty sessions of the first
   app can consume the entire default detail page cap before a later app's first
   session. Also lookup's BTreeSet iteration sorts IDs lexically, discarding supplied
   app-interleaved or selected-report priority. Interleave exact application buckets,
   retain first-seen outbound ID order, and test constrained budgets with a large
   first app and a later violating app. Inventory truncation still needs honest
   unknown coverage; no strategy can claim unseen groups were represented.
6. **Medium — profile posture extraction omits supported controls and invents others.**
   Recursive arbitrary string scanning misses supported
   `data-protection.data-leak-detection.action` because that object lacks a name,
   while unrelated nested strings equal to allow/block can become detector rows.
   Use known paths/named arrays and established active/shared directional semantics;
   include the existing directional DLP fixture and unrelated extension-field cases.
   Unsupported/ambiguous layouts must remain unknown, not establish monitoring.

Positive findings: request attempt admission includes token transport sends and
replay; operation slots cover body decoding and avoid nested OAuth semaphore
acquisition; deadlines/cancellation are scoped per invocation. SDK window limits
are disclosed, and exact 24-hour -> one-day conversion is used without approximate
queries. Direct scanner reads avoid management credential requirements. Missing,
duplicate and unrequested IDs are surfaced rather than treated as clean not-found.

Independent focused verification of the initial revision passed: 9 scanner-read
CLI tests, 2 required parity tests and 7 documentation consistency tests, with no
parity skip. The passing tests did not cover the independent failed-outcome and
metadata-echo probes. Root is revising these findings; a follow-up review and score
are required before Phase 4.

### Phase 3 follow-up inspection

The six functional findings have been revised and the independent original probes
now pass: a failed result returns exit 1/complete false with zero benign rows, and
DLP metadata echo no longer exposes its matched value. Root added targeted failed
outcome, mixed UUID, snippet-cap, application fairness, request-order and explicit
profile-path tests. Independent focused verification passed 14 CLI read tests,
2 parity tests, 7 docs-consistency tests, 18 signal unit tests and 2 budget tests,
plus formatting and all-target Clippy.

One final bounded-performance obligation was identified in the new PayloadSpans
projection. Exact identity checks used SecretSet.snapshot for every metadata field,
and SecretSet.redact also snapshots/sorts every form per call. At supported large
span/metadata counts this introduces quadratic cloning outside the collection
budget. Use exact membership without snapshot allocation and the existing optimized
carried_by matcher for negative tests; conservatively withholding a positively
matched payload field avoids repeated full-set replacement work. Root is resolving
this before the final phase score is issued.

### Phase 3 final re-review

Final score: **9/10 — phase gate passed**. No unresolved material Phase 3 findings.

The shared usable-result predicate now requires a known completed wrapper, explicit
clean error/timeout flags, an empty error array and known verdict/action values.
Discovery leaves failed/unknown outcomes unresolved and incomplete; direct views
normalize those outcomes without showing allow. An independent actual-CLI repeat
of the failed-result probe now returns exit 1, complete false and zero benign rows.
An additional uppercase-known-value probe correctly returned one threat row.

Invocation-local PayloadSpans now carries known DLP matches across excerpts,
untrusted metadata, collection and rendering, distinct from authentication SecretSet.
Typed semantic constants and short protocol-ID substrings are protected, while
whole/long sensitive matches are withheld. The original direct report metadata-echo
probe now returns no raw matched value. Exact-form membership uses a BTreeSet query;
negative/positive matching uses the existing optimized matcher instead of cloning
and sorting the registry for every field. The registry is bounded at 10000 unique
matches and 4 MiB raw text, with unsafe overflow refusing output. Independent
many-span and overflow tests passed; the previous performance concern is resolved.

Shared report normalization caps excerpts at 20 and marks truncation while retaining
variant counts; Unicode excerpts are bounded at 512 characters. The synthetic
collector now successfully handles a 21-excerpt report instead of failing at seal.
The exact 36-byte UUID grammar is reused before outbound batching, so unsupported
dashboard IDs do not poison valid neighbors. Missing request IDs remain distinct
from zero, and unsupported or ambiguous joins stay visible.

Discovery now interleaves exact application buckets and their session walks,
counts attempted pages against the aggregate bound, and preserves first-seen
outbound ID priority through lookup. Application-fairness and request-exhaustion
regressions passed. Sampling remains explicitly conditional on discovered
candidates; no claim is made about unseen groups in a truncated inventory.
Profile collection uses established active locations and explicit supported control
paths, including DLP and URL actions, with active/confidence metadata. Inactive,
ambiguous and unfamiliar layouts cannot establish monitoring from arbitrary nested
strings. Fixture-backed extraction and extension-field tests passed.

Independent verification of the corrected phase:

- Formatting and all-target workspace Clippy with warnings denied: passed.
- Required-parity CLI/docs run: 14 read tests, 2 parity tests, 7 docs-consistency
  tests passed; no parity skip.
- Signal unit suite: 18 tests passed before the final performance helper change;
  its 2 newly added many-span/overflow tests then passed independently. Two budget
  tests also passed independently.
- Original actual-CLI failed-outcome and DLP metadata probes repeated successfully
  against the corrected binary; all calls used the loopback harness.

This score qualifies Phase 3's bounded collection and investigation commands only.
Default live report replacement, private bundle publication, agent-review import,
full final workspace/MSRV/site validation and final adversarial review remain
Phases 4–6. No real tenant data, credentials, non-loopback API or SDK modifications
were used in this review.

## Phase 4 — default report integration and private bundles

Reviewer: independent Codex instance `/root/phase4_review`, 2026-11-02.
Reviewed the Phase 4 plan, DESIGN §18.14/§18.14.1, CLI validation, live/offline
orchestration, shared publication helper, bundle implementation, packet generator,
HTML additions and executable/parity coverage.

Initial score: **8/10 — revise before advancing**.

The default report correctly uses the shared evidence collector and keeps SDK
observations separate from administrator enforcement. Offline bundles verify
original evidence safety before copying immutable JSON; HTML-only projection remains
available. The no-replace rename, descriptor-relative writes, exact private modes,
file/staging/parent fsync handling and final-destination race tests are sound.
Independent reruns passed 30 report/offline/read acceptance tests and four bundle
unit tests; focused mandatory parity (eight tests) and Clippy logs also passed.

One cleanup defect was reproduced independently without tenant/network access:
under umask 0777, mkdirat(0700) creates a mode-000 directory, then read-only openat
fails before the staging guard exists. The command exits 1 and leaves an empty
`.airs-<uuid>.bundle` directory. Require early ownership/identity-based cleanup
covering open failures, with a restrictive-umask regression. Safe failure without
artifacts is sufficient; changing process-global umask is not appropriate.
Also request explicit report-level empty-discovery strict success and missing
scanner-credential coverage, and remove stale current-default/parity claims in
DESIGN/COMPATIBILITY. These are follow-up items before the final Phase 4 score.

Final re-review score: **9/10 — Phase 4 accepted**.

The early directory guard now captures ownership identity before opening and
removes only the still-matching empty staging entry on failure. A second independent
UID-1000 offline probe under umask 0777 now exits 1 with no staging or final bundle;
the new child-process regression pins the cleanup without changing the test
process's umask. A pre-rename inode/device check also refuses an already-replaced
staging name. Existing no-replace destination protection and post-publication
saved-data durability warnings remain intact.

The report-level gate now explicitly tests strict success after complete empty
session discovery despite unavailable supplemental daily telemetry, and missing
scanner credentials retaining management evidence with normal exit 0 / strict 1.
Updated focused acceptance results: 32 passed. The live-tier loopback adaptations
correctly check bounded discovery against the frozen TS stub and no-detail Signal
Brief output against the frozen schema-2 report; all four relevant self-tests pass.
These adaptations preserve isolated stores, no-content reads and secret checks;
they do not constitute agent-run live qualification. DESIGN and COMPATIBILITY now
identify the historical seven-source model and renderer as replaced by D340.

No remaining Phase 4 blocker. Phase 5 review ingestion, final full-workspace/MSRV/site
verification and the Phase 6 adversarial review remain separate delivery gates.

## Phase 5 — attributed Markdown review round trip

Reviewer: independent Codex instance `/root/phase5_review`, 2026-11-03 UTC.
Reviewed the approved Phase 5 scope, DESIGN §18.14.2 / D341, the strict JSON
frontmatter and marked-fence parser, CLI applicability/path guards, offline report
orchestration, reviewed HTML, immutable evidence and packet rebundling.

Initial score: **8/10 — revise before advancing**.

The parser is data-only, checks exact collection/digest/context bindings and known
unique finding IDs, and rejects duplicate JSON keys, unknown fields/enums and
ambiguous markers. Agent-assessed and human-reviewed statuses, self-attribution,
proposed priorities and unchanged API/count semantics are clearly separated.
Independent reruns passed three executable review round trips, two review unit
tests and five mandatory frozen-reference parity tests, without tenant/network
access. Input evidence and annotation files remain unchanged.

One concrete privacy gap blocks acceptance: `reviewer.reviewed_at` is validated as
an ISO timestamp but is not passed through the shared privacy projection, unlike
reviewer name/model/rationale. An independent synthetic offline probe planted
`2026-11-02T23:57:43Z` as a present DLP excerpt in valid re-digested evidence and
used the same timestamp as the bound packet's reviewer time. The snippet was
withheld, but HTML exited 0 and exposed that exact DLP value once in the attributed
`Time:` label. Require shared masking or safe refusal for tainted reviewer times,
with a regression and coherent packet reimport behavior. No real credentials or
service calls were used in this probe.

Final score after fixes: **9/10 — accepted**.

The reviewer timestamp now uses the shared authentication/payload projection in
both import and defensive rendering. A canonical withheld-time marker preserves
safe packet round trips when a timestamp is tainted; no date is invented. Raw
reviewer times are bounded to 120 bytes before ISO parsing, avoiding truncation
into invalid replay metadata. The documented packet schema and instructions match.
The original independent DLP-timestamp probe now exits 0 with no raw DLP value and
a withheld time label. An independent overlong fractional-ISO probe exits 1 with
empty stdout. Independent reruns pass four executable review tests and three unit
tests, including known-authentication-secret masking and sanitized packet reimport.
The mandatory parity and Clippy checks remain clean in the phase logs.

No remaining Phase 5 blocker. The full workspace, MSRV, documentation build and
final adversarial review are Phase 6 gates, not implied by this phase acceptance.

## Phase 6 — cross-cutting adversarial implementation review

Reviewer: independent Codex instance `/root/phase6_review`, 2026-11-02.
Reviewed the completed uncommitted implementation, approved plan, contributor
contract, DESIGN §18.11 and §18.14–§18.14.2, scanner/management client wrappers,
shared request budgets, normalized collection/correlation, privacy, analysis,
HTML, review packets and bundle publication. Inspected changes to shared report
helpers/date parsing and the loopback live-tier adapters as well as new tests.

Initial score: **8/10 — revise before delivery**.

The collection path keeps credential planes separate, resolves exact identities
without positional guessing, exposes partial coverage, preserves distinct counting
units and scan enforcement, and never submits scans or automatically retrieves
stored full content. The offline route skips tenant/client construction. Review
packets are data-only and bound to verified collection/digest/finding identities;
agent and human judgments stay attributed outside immutable evidence. Bundle writes
use private staging, descriptor-relative operations, durable no-replace publication
and ownership-sensitive cleanup; shared report behavior remains isolated.

Independent synthetic offline probes found two remaining privacy defects:

1. DLP spans were matched after control removal. A present DLP value containing a
   newline, repeated inside a PI excerpt, was displayed with the newline removed
   instead of withheld. The command exited 0. Require raw matching before
   normalization plus normalized-span registration to protect already-normalized
   echoes across all artifact routes.
2. IP recognition stripped hexadecimal letters/colons before parsing, so literal
   IPv6 addresses `fdab::abcd` and `fe80::ffff` were printed in PI excerpts. Require
   literal/bracketed IPv6 parsing along with existing IPv4 handling. Ensure control
   removal cannot create an unmasked IP/email in a single shared projection.

Both probes used only synthetic fixture evidence with recomputed valid digests,
private temporary files and cleared environments. No real tenant, credentials,
non-loopback request or SDK checkout was accessed. The root agent has applied fixes
and regression tests; final score follows independent verification of those fixes
and the remaining validation evidence.

Final implementation-review score after fixes: **9/10 — accepted**.

Shared payload processing now checks raw text before normalization and registers
control-stripped representations within the existing hard privacy bounds. The
independent original newline/ESC DLP probe and an already-normalized echo probe
both exit 0 without the raw or normalized sensitive value. Literal IPv6 addresses
are withheld too. Shared PII checks now inspect exactly the control-free text that
could be emitted, covering control-split IP/email values in a single projection.
Defensive rendering also completes evidence credential discovery before finalizing
a fresh review clone, avoiding earlier annotation leakage or truncation-prefix loss.

Independent reruns on the corrected implementation passed **29 Signal unit tests**
and **29 executable offline/read/review tests** (8 offline, 17 reads, 4 review).
Observed final Clippy and Rust 1.89 checks pass. The documentation site builds;
documentation consistency passes seven tests after two stale citations were fixed.
The first full required-parity workspace run had only that documentation-citation
failure and no parity skips; the root agent will record its final full-workspace
rerun separately before delivery. This review does not claim a pending check passed.

Chunk assessments: evidence/privacy/offline boundaries **9/10**; SDK collection,
correlation, budgets and counting **9/10**; HTML, review attribution and injection
boundaries **9/10**; private publication and shared-report isolation **9/10**;
contract/parity migration **9/10**. No remaining implementation blocker was found.
The preserved limits remain explicit: synthetic pinned-SDK compatibility is not
real-service qualification, daily token telemetry has no exact supported detail
window, arbitrary confidential prose cannot be universally identified, and review
attribution is not authenticated. Live validation, commits, pushes and releases
remain outside the implementation task.

### Phase 6 post-flight attribution audit

A final root-agent question prompted a new independent synthetic audit before
delivery. The earlier implementation-review acceptance is **reopened: 8/10 pending
an attribution fix**. This is not a test-suite failure; it is a confirmed semantic
collision in the privacy projection.

Three valid, re-digested synthetic offline snapshots each exited 0:

- A blocking profile named `blocker@example.invalid` at revision 1 and an unrelated
  allowing profile named `monitor@example.invalid` at revision 2 both projected to
  the same withheld name. An event using the blocker then incorrectly received the
  recommendation that current configuration supports monitoring, because analysis
  selected the unrelated higher revision by the projected name.
- Two distinct UUID scans sharing a DLP-matched UUID prefix became the same withheld
  scan ID. With distinct sessions/report IDs, validation accepted both events and
  HTML counted one distinct scan instead of two.
- Two exact application-name buckets containing different email-shaped names
  became the same withheld name and were grouped as one finding bucket.

Duplicate-full-identity and duplicate-profile-revision validation catches some
collisions, but not these distinct-session/revision cases. Privacy placeholders
must never become authoritative join/deduplication keys. Require an injective safe
identity representation or explicit fail-closed quarantine/refusal when correlation
or grouping identities are changed by projection, with regression coverage. No real
tenant or non-loopback service was used in these probes.

Post-flight fix review: **9/10 — accepted again**.

The final projection now retains a transient mapping from every projected
correlation/group key to its original value, including unchanged values. A
non-injective projection fails with the fixed operational error before publication;
placeholders can no longer join unrelated profiles, merge application buckets or
collapse scan counts. A single safely masked identity remains usable when equality
and inequality are preserved. Semantic source/window/direction/action/verdict/
confidence labels also may not change meaning through projection. The shared
untyped document path maps changed action/verdict/confidence text to `unknown`, so
direct report views cannot acquire an `allow`, `malicious` or `explicit` label by
removing controls.

Independent reruns of all three original collision probes now exit 1 with empty
stdout and no false monitoring recommendation. Separate unknown-action and
unknown-confidence normalization probes also fail safely. Six focused privacy
unit tests pass independently, including collisions against an unchanged literal
placeholder. The root-agent logs additionally show 32 Signal unit tests and 39
focused executable acceptance tests passing after this fix; Clippy remains clean.
The full required workspace run immediately before this attribution fix passed
3667 tests, zero failures, five ignored and no parity skips. The root agent must
record the final post-fix full rerun before delivery. No remaining attribution or
implementation blocker was found in this re-review; conservative refusal when
masking cannot preserve attribution is an intentional safety limit.

Final string-surface confirmation retains **9/10**. The shared text filter now
rediscovers authorization syntax after control removal, so a control-split Bearer
scheme cannot expose a token or leave an earlier echo unregistered. Direct-report
regression coverage also checks malformed detector action/verdict normalization.
All four formatted header timestamps now pass the same privacy filter before HTML
escaping, using protocol-time matching so short generic DLP values do not corrupt
public date provenance. An independent valid-digest fixture whose DLP value equals
its formatted reporting-window start now exits 0 with a withheld window label and
no sensitive date echo; its executable regression passes independently. Current
focused logs show 33 Signal unit tests and 41 acceptance tests passing, with clean
Clippy and MSRV checks. No remaining implementation blocker was found. Final
post-fix full-suite evidence remains the root agent's delivery gate.

## Final required-check completion (root delivery record)

After the final independent Phase 6 acceptance at 9/10 and all post-flight fixes,
`AIRS_REQUIRE_PARITY=1 cargo test --workspace --locked --no-fail-fast` completed
successfully: **3,674 passed, 0 failed, 5 ignored, no parity skips** (90 test groups,
including doctests). Formatting, Clippy with warnings denied, Rust 1.89 all-target
check and documentation build passed. This records executed validation, not an
additional self-issued independent review score. See SIGNAL_BRIEF_VALIDATION.md.
No real tenant, new scan, enforcement change, commit, push or release was performed.

## Separate release-candidate preparation review — 8.1.0-rc.1

Reviewer: independent Codex instance `/root/phase6_review`, 2026-11-03.
Scope: the user-authorized version correction and rebuilt validation packages,
without publication. This review is separate from the implementation-phase proofs
above; it does not relabel the earlier 8.0.0 working-tree test runs as RC runs.

Independent score: **9/10 — accepted**.

Cargo.toml and the root Cargo.lock package entry both identify 8.1.0-rc.1; no SDK
pin or dependency-lock churn accompanies the version correction. README current
status and source-build examples match, explicitly distinguish published stable
8.0.0 from this unpublished candidate targeting 8.1.0, and retain historical
benchmark/evidence producer versions. Candidate notes explain the feature scope,
compatibility differences, safety limits and remaining user-run qualification.

The README consistency check was strengthened: it now requires the status version
to equal Cargo's validated CARGO_PKG_VERSION, and still requires the install example
to match that status. Cargo validates SemVer, including prereleases, so removing the
old digit-only stable-version predicate does not relax version correctness. An
independent docs-consistency run passes all seven checks. The repeated required RC
workspace log contains 90 passing test groups: **3,674 passed, 0 failed, 5 ignored,
no parity skips**. RC Clippy, Rust 1.89 and documentation build records pass.

The source manifest has 253 relative, non-traversing paths; its current manifest,
lockfile and README hashes were independently checked. The private user live-bundle
path is absent from it and was not opened, copied or inspected during this review.
BUILD-INFO records the exact source-manifest hash, dirty working-tree provenance,
RC version, stable target, pinned SDK and unpublished status instead of claiming a
clean release commit.

Both requested archives were independently checked against SHA256SUMS and
BUILD-INFO binary hashes. They have exactly the expected executable, two compatible
symlinks and three text files, with root ownership metadata and 0755/0644 modes;
the archived README equals the current RC README. An independently extracted Linux
musl executable is static x86-64, and all three executable names print exactly
8.1.0-rc.1 under a cleared environment. The macOS archive payload matches the
checksum-verified Worf-native arm64 build; reviewed native smoke evidence passes
nine offline checks with zero network commands. The root's final extracted macOS
package record additionally verifies all three version aliases, strict ad hoc code
signature and the same offline workflow. macOS notarization is explicitly absent.

The finalized testing guide names the RC paths on enterprise and worf, marks the
older 8.0.0-labeled development packages superseded, and keeps publication separate.
No production Rust behavior was changed by this version-only preparation. No
remaining source/version/package blocker was found. No registry publication, tag,
commit, live tenant call or credential access was performed by this review.

## RC2 remediation phase 1 — collection scheduling

Independent Codex reviewer `/root/phase6_review`, 2026-11-03: **9/10 accepted**.
Independently reran six report-focused loopback tests successfully. Confirmed
configuration-first 20/70/100 cumulative reservations, three independent walks,
per-operation slices after acquiring slots (including hung profile/concurrency1),
deterministic merges, actual bounded detail batch concurrency and aggregate caps,
optional telemetry last and preserved cancellation/attempt/credential boundaries.
No phase1 blocker. Batches wait for their slowest peer before the next wave, as
specified; this does not promise complete traversal of large tenants.

## RC2 remediation phase 2 — metadata findings and detector aliases

Independent Codex reviewer `/root/phase5_review`, 2026-11-03: **9/10 accepted**.
Independent scoped checks passed 17 unit tests, 14 offline/review tests and the CLI
metadata-only/rate-limited example test. Findings/counts/IDs persist without report
examples; raw aliases remain auditable; rules 1 grouping, recommendations and packet
bindings remain intact. No phase2 blockers; report caps are reviewed separately.

## RC2 remediation phase 4 — exact transaction metadata fallback

Independent Codex reviewer `/root/phase5_review`, 2026-11-03: **9/10 accepted**.
Independent tests pass all five identity mismatches, explicit zero versus absent
subrequest, timeouttrue suppression, a 258-session inventory with aggregate/fallback
caps and a hung-read deadline. No status/extension-derived scan action/category or
report ID is imported; required scanner gaps remain. No phase4 blocker.

## RC2 remediation phase 3 — restrained Reports API use

Independent Codex reviewer `/root/phase6_review`, 2026-11-03: **9/10 accepted**.
Independently passed 26 read regressions and the repeated-operation transport guard;
reviewed all seven required parity cases. Verified default 5/hard10 IDs, max 2 actual
report sends across SDK operations, serial zero-retry reads and invocation-wide
429 latch, including response/error metadata handling. Direct lookup validates
before tenant access; optional examples cannot fail strict metadata completeness.
Rules 1 snapshots retain old limits offline; rules 2 caps at10. No phase3 blocker.
The nonblocking protocol-constant whitelist cleanup includes coverage-greedy-v2.

## RC2 final review — first audit and remediation

Independent Codex reviewer `/root/phase6_review` initially scored phase5 **8/10**
while final qualification was pending. A synthetic unknown profile detector with an
embedded control character could become a known alias after privacy filtering,
supporting an unjustified monitoring comparison. Candidate acceptance was held.

The fix requires unchanged protection/report detector labels and metadata hints
through privacy projection, not merely collision-free strings. Direct lookup maps
changed detector/service/direction labels to unknown. Explicit public protocol
spellings remain safe against payload-span masking; authentication-secret handling
is unchanged. New unit and offline publication regressions cover malformed controls,
hints and report labels, with direct-output regression coverage extended. Full
checks and all qualifying binaries are rebuilt after this fix. Final acceptance
and package evidence are recorded below once complete.

## RC2 remediation phase 5 — final qualification accepted

Independent Codex reviewer `/root/phase6_review`, 2026-11-03: **9/10 accepted**.
The prior 8/10 detector-normalization blocker is fixed and independently rechecked.
No unresolved material blocker remains within this unpublished candidate's scope.

The reviewer independently verified the exact 257-file source manifest and source
archive, including the generated build-support smoke helper; all workspace source
hashes match snapshot 03ef6829…f1cf. Both archives match SHA256SUMS, executable and
README hashes, exact six-entry layouts, relative aliases, 0755/0644 modes and root
ownership metadata. An independently extracted static x86-64 Linux binary and all
three aliases report 8.1.0-rc.2, pass both 11-check offline fixture workflows and
reject malformed control/hint/report aliases before publication.

Fresh required validation has 90 groups, **3,692 passed, 0 failed, 5 ignored and no
parity skips**; format, Clippy, Rust 1.89 and documentation build all pass. The
reviewer checked GNU and musl 26/26 optimized loopback logs, and final macOS native
build/hash provenance, extraction, strict ad hoc signature, both offline workflows
and malformed-alias refusal. macOS is not notarized. The reviewer performed no
private-bundle reads, live calls, credential access or source edits.

Final chunk scores: privacy/semantic guard 9; collection/budget/report restraint 9;
compatibility/docs 9; artifact provenance/offline qualification 9. All RC2 remediation
phases 1–5 are accepted at 9/10. Repeat real-service qualification remains user-run;
no commit, tag, publication or enforcement change was performed.

## RC3 profile matrix — implementation review

Independent Codex reviewer `/root/phase6_review`, 2026-11-03: **9/10 accepted**.
Seven HTML unit tests independently pass. The reviewer verified profile/revision
rows, canonical service columns, explicit active-state checks independent of action,
accessible inactive/unknown states, no absence-as-disabled inference, preserved raw
details, shared privacy and escaping, and 250-profile/32-service/250-detail caps.
Browser verification covers full desktop columns, mobile horizontal scrolling,
keyboard disclosure, zero network/page errors and printed page width. A separate
32-column synthetic case fits A4 and retains omitted services in expanded printed
details. Relative cell containers prevent hidden accessible text causing overflow.
The existing paired offline test and collected-report integration pin the matrix.
The integration now explicitly seeds a configured profile; an empty configuration
correctly produces an unavailable-inventory message instead of a false matrix.
Final RC3 checks and binary qualification are recorded after completion.

## RC3 final qualification — accepted

Independent Codex reviewer `/root/phase6_review`, 2026-11-03: **9/10 accepted**.
No unresolved material blocker remains in the matrix/presentation scope. The
257-input manifest 47138…eb94d matches the source archive and current build inputs.
Both archive checksums, executable/text payload hashes, layouts, symlinks, modes
and ownership metadata were independently verified. Extracted Linux aliases report
8.1.0-rc.3 and independently pass both twelve-check offline fixture workflows.
The final fresh macOS source/binary chain, strict ad hoc signature and combined
package workflows match; the binary is not notarized.

Final required validation passes 90 groups: **3,694 passed, 0 failed, 5 ignored and no
parity skips**. Format, Clippy, Rust 1.89 and site checks pass. Optimized GNU and
musl each pass 26/26 SDK loopback checks. Matrix source/browser/print acceptance and
the concurrency fairness assertion correction independently remain at 9/10.
No source edits, private payload reads, credentials or live calls were performed
by the reviewer. The acceptance does not claim to fix missing scan results in the
user's RC2 collection. No commit, release tag or publication occurred.
