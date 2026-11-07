# AIRS Signal Brief implementation plan

Status: initial feature implementation and independent reviews complete; RC1 live
qualification exposed open blockers. User-approved remediation is recorded in
SIGNAL_BRIEF_RC2_REMEDIATION.md. Every phase 0–6
scored 9/10. Final full-workspace validation passed (3,674 tests, no failures or parity skips); see
SIGNAL_BRIEF_VALIDATION.md and SIGNAL_BRIEF_PHASE_REVIEWS.md. Prepared 2026-11-02
from AIRS_SIGNAL_BRIEF_PROMPT.txt, the scope interview, and the repository. Release,
real-service qualification and publication remain outside this implementation.

## 1. Approved scope and precedence

The scope interview supersedes two parts of the original prompt: replace the HTML
produced by `airs runtime report` rather than adding another report command, and
replace the nonfunctional `runtime scan-logs query` stub with working discovery.
The rest of AGENTS.md and the original evidence, privacy, and SDK constraints apply.
Update DESIGN and the compatibility records with each observable implementation
change; do not quietly keep tests asserting the former HTML as the current output.

Deliver:

- A standalone, accessible, printable security brief with executive summary,
  detector volumes, prioritized grouped findings, monitoring-to-blocking review
  recommendations, current profile comparisons, unreviewed/noise analysis, and gaps.
- Working scan discovery, results lookup, and detailed report lookup using SDK
  methods. Collection and rendering use the same normalized evidence pipeline.
- Default rolling 24 hours, configurable hours/days, tenant-wide by default with
  repeatable application-ID filters. No arbitrary historical live time ranges.
- Automatic bounded violation-snippet collection; masked artifacts, with DLP
  matched values entirely withheld. Never automatically fetch stored full content.
- Deterministic coverage-oriented detail selection, 100 distinct detailed reports,
  120 seconds of collection and four requests in flight by default.
- An optional bundle containing HTML, versioned masked evidence JSON and a Markdown
  agent-review packet. Offline rerendering optionally imports an annotated packet.
- API observations, CLI derivations, agent assessments and human review remain
  distinct. Detector violations imply malicious under the understood service
  semantics; enforcement action remains independent. Unknown values stay unknown.
  Malicious + allow is not automatically a misconfiguration. Monitoring claims need
  supporting configuration; current configuration is not historical event policy.

Non-goals: profile/tenant/enforcement mutations, remediation, new scans, autonomous
agent execution, arbitrary customer-code execution, owner-roster integrations,
historical drift between snapshots, publication, commits/pushes/releases, SDK edits
or dependency changes away from crates.io `scm-sdk =0.2.4`. No real live tests are
agent-run. Existing explicit `sessions scan-content` remains the full-content route;
this project does not add full-content inclusion to shareable briefs.

## 2. Grounding and constraints

Inspected integration points:

| Location | Existing behavior and intended reuse |
| --- | --- |
| `src/commands/runtime_report.rs` | Seven sequential daily sources; replace its report orchestration, preserve destination and output conventions. |
| `src/commands/report_command.rs` | Preflight, SourceBudget, publication and strict exits; adapt through runtime-specific helpers without changing other reports. |
| `src/report/runtime.rs`, `runtime_html.rs` | Current schema 2 intentionally drops identities and content; unsuitable as the new evidence format. Reuse sound parsing/pagination primitives, not the lossy model. |
| `src/report/html.rs`, `src/files.rs` | Escape/CSP patterns and private no-clobber publication. Single-file reservations do not by themselves provide atomic bundle publication. |
| `src/cli.rs`, `src/commands/runtime_extras.rs`, `runtime_inventory.rs` | Report flags, plan validation, tenant connection and broken scan-log stub. |
| `src/clients.rs`, `src/runtime/*` | Plane-specific clients, recorded OAuth leases, endpoint guards, read budgets and sanitized views. |
| `crates/airs-test-harness`, `tests/runtime_report.rs`, `tests/parity_runtime_report.rs` | Loopback fixtures, sandbox path guards, executable and dual-CLI tests. |

SDK inspection found `Scanner::query_by_scan_ids` and `query_by_report_ids` accept
1–5 IDs per request. Results and reports have optional `scan_id` and `req_id`;
reports also have `report_id`, session and transaction identities. Dashboard session
actions and transactions expose `scan_sub_req_id`. Never equate these fields just
because their names look related: verify joins against synthetic contracts.

The local SDK source is read-only reference. Verify capabilities against the actual
Cargo registry source for `scm-sdk-0.2.4` before using them; newer checkout methods
are not evidence that the pinned crate supports them. Snippet families are
`pi_snippets`, `dlp_snippets.snippets`, `dbs_snippets`, `tc_snippets`. Availability
varies; absent, empty, withheld and unavailable are different states.

The visual reference at `/home/cdot/Downloads/AIRS Signal Brief.html` has a JSON
string in `script[type="__bundler/template"]`. Parse as data. Borrow hierarchy and
layout, never its tenant content, counts, dates or claims. Private SDK qualification
files are not fixture inputs. The preparation under `.local/dev/signal-brief/` is
ignored tooling, not a committed feature dependency.

## 3. Concrete command and artifact contract

```sh
# Live collection, HTML only by default
airs runtime report --interval 24 --unit hours --output-file ./brief.html
airs runtime report --interval 7 --unit days --app-id APP_A --app-id APP_B

# New private directory; fixed filenames report.html, evidence.json, review.md
airs runtime report --bundle-dir ./brief-review

# Strictly offline; no tenant lookup or API clients, even with --tenant present
airs runtime report --from-evidence ./brief-review/evidence.json \
  --review-file ./brief-review/review.md --output-file ./reviewed-brief.html

# Shared discovery and ID-based investigation
airs runtime scan-logs query --interval 24 --unit hours --output json
airs runtime scan-logs query --interval 1 --unit days --app-id APP_A --filter threat
airs runtime scan-logs results --scan-id ID_A --scan-id ID_B --output json
airs runtime scan-logs reports --report-id REPORT_A --output json
```

Retain report `--title`, `--strict`, `--max-pages` (default 40, range 1–100), default
timestamped HTML naming, `--output-file -`, quiet behavior and HTML-only format
policy. Proposed new live controls: `--max-reports` (default 100, 0–1000; zero
explicitly disables detail sampling), `--timeout` (seconds, default 120, 1–3600),
`--concurrency` (default 4, 1–16), and `--max-requests` (default 1000, 1–10000).
The request cap includes data calls, OAuth attempts and retry/refresh attempts;
instrument existing transports/providers rather than silently counting logical calls.

Live window intervals are positive integers with `hour|hours|day|days`; enforce each
SDK method's narrower supported vocabulary/bounds, convert equivalent units only
when exact. Unsupported source windows are gaps, not substituted daily metrics.
Record actual query windows rather than claiming fixed timestamps the service did
not accept. Locally filter usable event timestamps to the recorded half-open target
window; quarantine missing/out-of-window timestamps from window-specific totals.

`--bundle-dir` is exclusive with `--output-file`; offline rendering can also emit a
new bundle. `--review-file` requires `--from-evidence`. Offline mode rejects explicitly
supplied live scope/budget flags; it uses the saved scope and coverage. An inherited
tenant selection is ignored offline. Validate usage and all destinations before
tenant access, and validate bounded offline input/schema before any publication.

Discovery defaults to all events (not only threats); retain and validate current
`--filter all|benign|threat`, `--limit`, `--offset` and required interval/unit syntax.
Define `--all` and bounded traversal explicitly. Dashboard offsets are not a flat
scan-log cursor: limit/offset describe the normalized deterministic collected event
view, with collection coverage exposed separately. Reject deprecated `--page` and
`--page-size` with a migration message instead of silently ignoring them. Document
that pagination across separate invocations is not a stable snapshot; use saved
evidence for stable offline investigation. Results/reports accept repeatable IDs,
batch at five, deduplicate outbound IDs while retaining requested-ID correlation,
and print normalized masked data in existing pretty/table/json formats. These
commands build only the needed credential plane; direct report/results lookup does
not require management OAuth. Missing requested records are not a successful empty
lookup; unmatched or ambiguous rows remain visible and yield incomplete lookup exit 1.

Option applicability (scalar flags reject repetition; only IDs/app-ID filters repeat):

| Command/mode | Applicable options |
| --- | --- |
| Live report | interval/unit (default 24/hours), app-id, max-pages, max-events, max-reports, timeout, concurrency, max-requests, title, strict, output-file OR bundle-dir |
| Offline report | from-evidence, review-file, title, strict, output-file OR bundle-dir; reject explicitly supplied live controls |
| scan-logs query | required interval/unit, app-id, filter, limit, offset, all, max-pages, max-events, timeout, concurrency, max-requests, standard output/quiet; no max-reports or snippets |
| scan-logs results/reports | respectively scan-id/report-id, timeout, concurrency, max-requests, standard output/quiet; reject discovery/window/filter/page flags |

`--max-events` defaults to 10000, range 1–10000, and caps normalized events across
all walks; exhaustion is partial discovery. `--max-pages` aggregates page requests
across each endpoint family, including all session-detail walks, not per session.
Transaction enrichment consumes the global attempt budget. Application/session walks
use round-robin scheduling with a one-page quantum.

Query --all defaults false. Without it, limit is 1–100 (default 50), offset is
0–9999 (default 0). With --all, explicit limit/offset are usage errors. Discovery
remains bounded; resolve candidate scan results, sort event time descending (missing
last), then exact app/session/scan/sub-request identity. Apply verdict filter,
offset, then limit. Benign/threat filters require explicit corresponding API
classification; missing/failed/unknown results never become benign. The output
envelope includes items, requested slice, limits, discovery and resolution coverage.
A complete slice does not imply complete discovery; partial discovery or unresolved
candidates exits 1 after writing available data. Reports reuse the service without
inheriting query's output/exit policy.

Direct commands accept 1–1000 IDs, validate UUID scan IDs and the pinned SDK's
1024-byte opaque report-ID bound (no commas/controls) before tenant reads, and batch
at five unique IDs. Time/concurrency/attempt defaults apply to all three commands.
View/string/snippet bounds also cover direct outputs. Test the applicability table.


## 4. Pipeline and invariants

Proposed modules: `src/report/signal/{mod,evidence,collect,correlate,privacy,analyze,
html,review,bundle}.rs` plus a shared `src/runtime/scan_logs.rs` service and
`src/commands/runtime_scan_logs.rs` adapter. Adapt names to existing conventions;
avoid a second HTTP stack or refactoring unrelated report families.

```text
SDK reads -> transient typed observations -> correlate + privacy projection
          -> versioned evidence -> deterministic analysis -> HTML / review packet
saved evidence + validated review packet -----------------> analysis -> HTML
```

Evidence schema v1 stores schema/rules/redaction versions, collection ID, CLI/SDK
version, tenant display label, requested app scope, window, source timestamps and
queries (without credentials or URL queries), budgets, completeness, sample ledger,
normalized observations, associations, identities, and provenance. Every metric has
a unit, population, window, completeness, derivation and source/evidence references.
No raw SDK response dumps, arbitrary extension maps, headers, endpoints or credentials.
Preserve relevant unfamiliar detector labels safely, not unknown response members.
Reject duplicate JSON object keys before canonicalization so hashing and validation
interpret the same structure. Validate schema versions, duplicate identities, dangling references, enums, count
bounds, timestamps, collection binding, file size (32 MiB evidence, 4 MiB review),
nesting depth and string lengths. Unsupported major versions fail operationally.

Canonical evidence bytes are UTF-8 JSON with lexically sorted object keys, array
order preserved, no insignificant whitespace, and a documented finite-number
serializer. SHA-256 excludes the top-level digest member. Review/render metadata is
outside evidence. Verify the input digest before additional privacy projection;
whitespace/property-order changes must not alter it. Retain supported generating
analysis rules for replay; refuse unsupported rules/redaction versions. A newer
mandatory privacy projection is labeled with original/applied versions and may
withhold more text, never mutate or re-digest the source. A digest proves integrity,
not authenticity: treat even correctly digested saved evidence as attacker-controlled.
Offline bundles preserve original evidence only if shared privacy validation confirms
it is safe to republish unchanged. If extra projection is necessary, refuse bundle
output with a fixed sanitized error before publication, leave the source unchanged,
and permit safe HTML-only rendering. Do not copy unsafe original JSON into a bundle.
Validated review records remain in review.md, outside immutable evidence. Tests cover
tampering, canonical ordering, repeated replay, upgraded privacy projection, and an
attacker-recomputed valid digest containing planted sensitive strings: safe HTML-only
output is possible, but bundle publication is refused and raw JSON is never copied.


Correlation rules: discovery session identity is `(app_id, exact_app_name, session_id)`;
observed dashboard name buckets remain distinct from registered applications. App-ID
filters include all matching name buckets; cross-bucket associations require explicit
evidence. Event identity includes scan ID and validated
sub-request identity, and application/session where supplied. Report ID identifies
retrieval, not a detector occurrence. Distinguish scan results, detector rows and
occurrences. Preserve duplicate sightings as provenance, not additional events.
Do not coerce absent req_id to zero or merge unordered batch results by position.
Ambiguous/missing identities create explicit unresolved links. Source-population
metrics never silently derive from the bounded report sample.

Normalize prompt, response, tool-call, tool-response and unknown directions
separately. Group findings by `(application, observed profile, detector, direction,
scan_enforcement_action)`, preserving unknown association states. Findings retain all supporting
identities, collected occurrence count and distinct-scan count; sampled distinct
payload variants are explicitly not population cardinality. Use collection-local
opaque payload-variant IDs computed in memory before redaction; persist neither raw
payloads nor reversible encodings/unkeyed sensitive-value hashes. Finding IDs derive
from stable normalized grouping keys; annotations additionally bind to collection
and evidence digest and are not automatically reusable across collections.

Persist scan-level verdict/action, detector-row verdict/action and current profile
configuration separately with provenance. Grouping/sampling use observed scan-level
enforcement only. Missing scan action stays unknown even when a detector/current
policy supplies one. Preserve conflicts and suppress definitive enforcement
recommendations. Only positively identified violation rows enter violation counts;
benign/unknown rows remain context. Test malicious PI plus benign DLP, disagreeing
actions, absent scan results and multiple detector actions for a scan.


Privacy projection precedes every durable/output surface, including direct commands,
errors, packet generation and synthetic debug records. SecretSet remains the
credential backstop; record all OAuth leases. Fully suppress DLP snippets/matched
values and any other excerpt containing the same known sensitive spans. Preserve
non-sensitive pattern metadata only through an allowlist. Bound other excerpts to
512 Unicode characters after redaction, and remove controls. Obvious credentials,
tokens and personal identifiers are redacted across every string field. If safe
projection cannot be established, withhold the excerpt. Redaction does not promise
universal classification of arbitrary confidential prose; disclose residual
sensitivity and keep all artifacts private. Reapply offline safety checks to
untrusted saved evidence and annotations so a forged sidecar cannot bypass masking.

Sampling: enumerate bounded candidate events, form deterministic strata by
application/detector/action, sort each by timestamp descending then identity, and
select round-robin with stable stratum ordering. Selection is coverage-oriented,
not a statistical random sample. Since detector/action may only appear after
enrichment, first use available session/transaction labels; unresolved candidates
form an explicit unknown stratum. Report IDs shared across strata consume one slot;
selection advances to fill remaining unique slots. Persist candidates seen, eligible
counts, selection order, omitted/uncovered strata and algorithm version. If discovery
is partial, sample coverage applies only to discovered candidates. Missing snippets
are normal availability states; failed selected-report retrieval is incomplete.

Analysis is pure and versioned. Rank on explicit detector evidence, recurrence,
application reach and review status with documented tie breaks; action alone never
proves misconfiguration, attacker intent, false positive, leakage to other users or
blocking readiness. Recommendations link evidence and rationale. Compare current
profiles directionally, including inherited/default/unknown behavior; keep current
posture separate from event-time profile attribution. Unknown mappings prevent
specific configuration recommendations. No historical drift or owner guesses.

Collection uses independent management and scanner clients for the same resolved
tenant, one cancellation context and a global monotonic deadline. Missing scan
credentials disable that source without suppressing valid management evidence.
Queue length, response size, normalized event count (default ceiling 10000), pages,
requests and concurrency are bounded; document ceilings and count every upstream
fan-out. Lack of timestamp validation or pagination advancement never becomes clean
coverage. Retries and OAuth replay consume both time and request budgets.

Reserve discovery/enrichment/detail stages at 40/30/30 percent of time and outbound
attempts (default 48/36/36 seconds and 400/300/300 attempts). Earlier unused allowance
carries forward; monotonic stage cutoffs and the global deadline remain authoritative.
Integer attempt partitions sum to the cap; a zero-slot stage is uncollected, never
bypassed. Optional charts/trends use spare resources after required detail work.
Actual HTTP sends, including OAuth/replay, share invocation-local atomic admission
and concurrency semaphores. Rendering/publication are outside the collection timer.
Query has discovery then resolution stages, carrying the detail allowance into
resolution; direct commands use one stage with the whole budget. Test slow/large
first applications, tiny caps, cancellation while queued and useful detail progress
or an explicit no-resolved-IDs gap. Truncated discovery is partial, not representative.


## 5. Delivery phases and chunks

Chunks are reviewable increments with their own tests and corresponding contract
updates. Never merge dead-end command scaffolding that returns fabricated success.
The ordering deliberately delivers offline behavior before broad network traversal.

### Phase 0 — contracts and SDK feasibility

**0A. Verify the pinned capability chain.** Inspect actual registry 0.2.4 session,
transaction, result and report types, query bounds and direction fields. Build a
source-to-field matrix in `docs/research/rs-sdk-signal-brief.md`, including ways to
obtain report IDs from session action -> scan result -> report, UUID restrictions,
sync versus async IDs, and req_id/sub-request relationships. Add a minimal loopback
fixture chain testing supported join rules without private samples. No real tenant reads.
Deliverable/gate: a pinned-SDK compatibility test yields a correctly correlated
synthetic snippet. It does not qualify live equivalence between req_id and
scan_sub_req_id. Document the basis for each join (pinned contract, existing CLI
correlation, user-provided qualification summary); match exact supplied identities
only where that basis supports equivalence. Unsupported sync/async variants remain
unresolved and are listed as real-service qualification limits. If any required join is not supported,
produce a synthetic reproducer, mark the source blocked and continue offline work;
do not invent the link, bypass SDK or declare collection complete.

**0B. Freeze proposed contracts.** Prepare proposed DESIGN §18.11/§18.14 changes,
CLI examples, milestone scope and divergence ledger in a marked research spec.
Promote each behavior into binding DESIGN and COMPATIBILITY only with its
implementation, executable tests and dNN tests in that same chunk. Allocate
unused D numbers only when implementing their tests; do not assume D337 is free.
Document every option conflict, bound, output projection, strict rule and masking
rule above. Record that runtime HTML is an authorized product divergence from the
frozen TypeScript spec. Gate: matrix maps every new behavior to its implementation
chunk and proposed executable coverage; no claims of implemented status yet.

### Phase 1 — normalized evidence and safe offline foundation

**1A. Evidence and correlation model.** Implement v1 types, canonical serialization,
reference validation, explicit missing/empty/withheld states, provenance and typed
join rules. Add independent synthetic evidence for all four snippet families,
unknown detectors, multi-row async results, Unicode and missing identities.
Tests: shuffled results, duplicate IDs, req_id zero versus absent, conflicting
application IDs, same session across applications, same app/session IDs in two exact-name buckets
with a different registered-app name, one scan with several requests,
unmatched reports and exact counting units. Gate: valid evidence round-trips and
invalid identity graphs fail without printing source text.

**1B. Privacy projection.** Add field allowlists, DLP suppression, credential/PII
redaction, content caps and control handling; keep raw observations transient.
Tests plant secrets in every model field, extensions, errors and mixed snippet
families, including a DLP value repeated inside a PI snippet. Confirm no raw values
or sensitive hashes appear in JSON, terminal views, HTML or Markdown. Gate: a
single shared safe projection is mandatory for every artifact route.

**1C. Bounded input and offline command routing.** Implement `--from-evidence`,
preflight, input validation and pure analysis entry point independently of tenant
loading. Add harness path flags for every new file/directory option and repeated
occurrence. Tests: unreadable/malformed/oversized/unsupported schema, symlink and
FIFO behavior per file conventions; nonexistent tenant, poisoned credentials and
non-loopback endpoints do not affect offline success. Gate: offline execution
constructs no clients and makes zero fixture requests.

### Phase 2 — analysis and replacement HTML, offline first

**2A. Findings and methodology.** Implement grouped findings, explicit ranking,
aligned metric denominators, verdict/action separation, monitoring context,
configuration comparisons and evidence-backed recommendations. Add table-driven
synthetic cases for missing associations, profile revisions and all four directions.
Gate: unreviewed data never becomes a claimed false positive/confirmed attack;
allowed detections alone do not produce enforcement-change instructions.

**2B. Standalone renderer.** Build the new visual hierarchy from the reference's
layout without its content. Use native details/summary where possible, self-contained
assets, CSP-hashed optional scripts, keyboard controls, skip link, contrast and print
CSS. All evidence remains readable without JS. Keep unsupported sections visibly
unavailable. Tests: adversarial HTML/script/Markdown text on every surface,
JavaScript-disabled rendering, no asset URLs/network callbacks, CSP hashes and
bounded document growth. Produce a committed synthetic evidence example and HTML
generated through the real offline CLI. Gate: inspect desktop/mobile/print output;
do not count screenshot creation alone as interaction/accessibility validation.

### Phase 3 — bounded live collection and working scan-log commands

**3A. Shared discovery.** Implement the dashboard/session traversal service with
window normalization, application filtering, timestamp eligibility, per-source
coverage and hard global limits. Reuse existing SDK readers and pagination checks
where sound. Tests: short/full pages, repeated/non-advancing pages, total changes,
overlap, cap exhaustion, stale timestamps, denied/empty/missing collections,
incomplete later applications and app-scoped versus tenant-wide metrics.
Gate: `scan-logs query` works against the fixture and never reports an incomplete
traversal as complete or not-found. Remove the broken-description only alongside
the working implementation and intentional-difference tests.

**3B. ID results and reports.** Implement scanner-only direct commands, five-ID
batching, missing-ID ledger, typed view projection and ordered identity joins.
Test scan-only tenants, management-only tenants, no credential crossover, rejected
UUIDs/opaque report IDs, mixed found/missing answers, unknown status/action, retry
budgets, and malicious allowed versus blocked. Gate: commands expose all supported
snippet families safely without retrieving full scan content or requiring OAuth.

**3C. Sampled report collection.** Connect discovery, optional transaction enrichment,
scan results and selected report fetches. Use bounded queues and shared deadline;
all prerequisite enrichment consumes request/event/page budgets. Make sampling
reproducible given the same candidate evidence. Test stratification starvation,
more strata than slots, repeated report IDs, unavailable detector/action metadata,
out-of-order futures and results, partial batches and deadline during OAuth.
Gate: changing scheduling cannot change analysis ordering for identical evidence;
coverage labels never imply the 100-report sample is the population.

### Phase 4 — live report integration and publication

**4A. Replace default orchestration.** Wire the new collector -> evidence -> analyze
-> HTML pipeline into `runtime report`. Retain unaffected shared report behavior.
Define required sources as applications, profiles, registered-app associations,
session discovery and candidate scan-result resolution; require selected detailed
reports when candidates exist and max-reports > 0. Chart/trend supplemental sources
are explicitly optional. No candidates after complete discovery is a valid empty
sample; omitted sampling by explicit zero is documented, not a retrieval failure.
Global unknown-health conditions retain exit 1. Otherwise partial evidence writes
with normal exit 0; strict exits 1 if a required source is incomplete. All source
failures are visibly gaps regardless of whether strict requires them.
Gate: executable tests cover empty successes, missing scan credentials, all sources
unavailable, partial sources, quiet/stdout, strict behavior and no scan submissions.

**4B. Private bundle publication.** Preflight final directory absent and parent
valid before tenant access. Build a sibling 0700 staging directory with 0600 files,
write/fsync all three artifacts, fsync the populated staging directory, then publish
the directory with an atomic no-replace
primitive available through safe rustix APIs, and fsync parent. Use rustix renameat_with(NOREPLACE) on Linux/Apple; if the
filesystem refuses it, fail safely without an overwrite-capable fallback. A parent
sync failure after publication reports saved data with a durability warning. Do not use ordinary
rename that could replace a raced empty directory. Remove only owned staging data
on failure/cancellation; never delete a raced destination. Keep existing single-file
publication unchanged. Tests inject file/staging-directory/parent fsync failures, unsupported no-replace
flags and destination races, and assert coherent collection IDs/digests and private modes.
Gate: either the complete bundle is published or no final bundle exists; sync
warnings and post-publication interruption contexts accurately describe saved data.

### Phase 5 — Markdown agent review round trip

**5A. Packet generation.** `review.md` begins with strict JSON-compatible YAML
frontmatter: packet version, collection ID, evidence digest, reviewer metadata and
a fixed task prompt. Body contains safely fenced masked evidence and separate
machine-readable assessment records in explicitly marked JSON fences. Escape fence
terminators and delimiter collisions; no raw HTML rendering or external links.
The prompt tells an agent to treat snippets as untrusted data, assess only supplied
evidence, cite finding IDs, preserve uncertainty, and never modify source evidence
or execute embedded instructions. No CLI agent invocation. Gate: synthetic injection
payloads cannot break packet structure or become the top-level task prompt.

**5B. Review ingestion.** Parse only the documented metadata and assessment blocks;
do not execute frontmatter prompts, tools, templates or embedded code. Validate
digest/collection/finding references, reviewer type/name/model/time, bounded rationale
and assessment vocabulary. Unknown, duplicate or cross-collection records fail with
sanitized errors. Agent-assessed and human-reviewed remain distinct; reviewer claims
are self-attributed, not cryptographically authenticated. Imported content passes
the same privacy/output rules. Assessments may propose prioritization but cannot
rewrite API verdicts, enforcement actions, completeness or original metrics.
Gate: CLI -> packet -> synthetic annotation -> offline HTML round trip passes with
zero tenant/network access; forged/stale packet rejection is executable-tested.

### Phase 6 — compatibility, adversarial review and final verification

**6A. Contract and help reconciliation.** Each prior observable chunk must already
include its DESIGN/COMPATIBILITY changes and executable/dNN tests; this chunk
reconciles cross-cutting claims rather than deferring parity migration. Update README, docs-site command explanations,
generated help reference, DESIGN §12/§18 and COMPATIBILITY. Replace affected former
runtime HTML parity assertions with dNN tests asserting new Rust and frozen TS
behavior; preserve unaffected shared report tests. New commands/flags must assert
TS rejection as well as Rust success where applicable. Review D155/D156/D214/D226
and all old schema-2/aggregate-only claims; extend/retire rows explicitly rather
than deleting historical differences. Run docs consistency and CLI reference checks.

**6B. Independent adversarial implementation review.** Review the completed diff,
not just this plan, for credential crossing, secret leakage, agent-prompt boundary,
unsafe correlation, optimistic coverage, cardinality errors, offline network access,
publication races, timer/cancellation leaks and unintended changes to other reports.
Fix confirmed findings and add regression tests. Score each major chunk and the
whole implementation using the repository's applicable review practices, separately
from the plan's requested 1–10 score. Never call the release milestone complete:
live validation, push and release are outside this authorization.

**6C. Required validation and delivery record.** Run formatting, Clippy, full parity
suite, 1.89 check and docs build. Record exact outcomes, source limitations,
changed commands, example generation and independent review results. Explicitly
separate loopback proof from real service qualification. If desired, supply a
documented user-run read-only validation procedure using the existing isolated live
tier; do not run it and do not reuse a live test that submits new scans.

## 6. Dependencies and acceptance matrix

Execution order: 0A/0B -> 1A -> 1B -> 1C -> 2A -> 2B; then 3A -> 3B -> 3C ->
4A -> 4B -> 5A -> 5B -> 6A -> 6B -> 6C. Fixture preparation and documentation
research may overlap independent work, but do not parallelize edits to shared CLI
or contract files without an explicit ownership boundary.

| Requirement | Primary chunks | Acceptance evidence |
| --- | --- | --- |
| Pinned SDK and valid joins | 0A, 1A, 3B/3C | Synthetic multi-request chain and ambiguous-link tests |
| Reproducibility and offline isolation | 1A/1C, 2A, 5B | Canonical evidence and zero-tenant/network executable tests |
| Privacy across all outputs | 1B, 2B, 3B, 5A/5B | Planted-value tests across artifacts and error/debug paths |
| Honest counts, verdict/action and monitoring | 2A, 3A/3C | Unit/window/population and current-versus-event policy cases |
| Working discovery and lookup | 3A/3B | Loopback CLI tests, plane isolation, matched/missing IDs |
| Sampling and bounded fan-out | 3C | Ledger, starvation, request/deadline/concurrency tests |
| Partial, strict and interruption behavior | 3C, 4A/4B | Deadline, signals, missing credentials, strict-exit and race tests |
| Portable accessible HTML | 2B | CSP/injection tests and desktop/mobile/print/manual control review |
| Agent review round trip | 5A/5B | Digest binding, status separation, injection and malformed review tests |
| Contract and frozen reference differences | 0B, 6A | dNN tests asserting both executables plus docs consistency |

Required final commands, using the prepared local tools:

```sh
mise exec -- scripts/build-reference-cli.sh --verify
mise exec -- cargo fmt --all --check
mise exec -- cargo clippy --workspace --all-targets -- -D warnings
mise exec -- env -u AIRS_RS_LIVE -u AIRS_E2E_TENANT -u AIRS_E2E_PROFILE \
  -u AIRS_ALLOW_PARITY_SKIP AIRS_REQUIRE_PARITY=1 cargo test --workspace
cargo +1.89.0 check --workspace --all-targets --locked --target-dir target/msrv
mise exec -- pnpm -C docs-site build
```

Prior host-setup baseline: 3601 tests passed, 0 failed, 5 ignored; no parity skips;
fmt, Clippy, MSRV and docs build passed. These are baseline results only, not
validation of this future implementation.

## 7. Plan review gate

Before feature implementation, an independent Codex instance reviews this plan and
the repository, scores 1–10 overall and for scope, feasibility, correctness/privacy,
delivery sequencing and validation. Target overall >=9 with no unresolved critical
or high-severity feasibility/correctness issues. This threshold is a planning
quality bar, not user approval or permission to run live tests or publish.
Record the review and any revisions in `docs/SIGNAL_BRIEF_PLAN_REVIEW.md`; request
a re-score after material changes. Resolve small engineering details from code;
escalate only actual scope changes or a concrete SDK blocker.

Independent review completed: initial overall 8/10, revised overall 9/10 (all five
dimensions 9/10), no unresolved critical/high plan findings. Review and resolution
history: [SIGNAL_BRIEF_PLAN_REVIEW.md](SIGNAL_BRIEF_PLAN_REVIEW.md). This passes the
plan review gate only; implementation progress is recorded in the status above and
in the per-phase review ledger.
