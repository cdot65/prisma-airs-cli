# Independent Codex review: AIRS Signal Brief implementation plan

Review date: 2026-11-02. Reviewer: independent Codex agent
`/root/signal_plan_review`. Reviewed the initial 435-line revision of
`docs/SIGNAL_BRIEF_IMPLEMENTATION_PLAN.md`, the original task prompt, the approved
scope interview, AGENTS.md, and relevant CLI and pinned SDK source. This is a plan
review, not implementation validation. No features, SDK files, private customer
evidence, live services, or credentials were modified or accessed.

## Initial score and decision

| Dimension | Score / 10 | Reason |
| --- | ---: | --- |
| Scope fidelity | 9 | Captures the approved replacement, repaired discovery, masking, sampling, offline workflow and attributed agent review. |
| SDK/implementation feasibility | 8 | Required typed APIs and safe atomic publication primitives exist; identity mapping and budgets need sharper contracts. |
| Correctness and privacy | 7 | Strong defensive coverage, but application-bucket identity and action-level ambiguity could produce incorrect findings. |
| Delivery sequencing | 8 | Offline-first delivery is sound; late operational details and collection starvation need resolution. |
| Validation | 9 | Good loopback, parity, privacy, cancellation, adversarial and visual checks; a few acceptance cases need additions. |
| **Overall** | **8** | **Revise before feature implementation.** |

The requested review occurred before implementation. The plan's own >=9 gate has
not passed. There is no demonstrated SDK blocker, but the two high-priority design
issues below should be corrected and independently re-scored first.

## Findings

### H1. Preserve dashboard application-name buckets in discovery identities

Plan lines 157–164 use `(app_id, session_id)` and retain application name only as
query metadata. Existing collection deliberately keys sessions by
`[application_id, application_name, session_id]` (`src/report/runtime.rs:666`), and
its documented limitations explain that literal scan metadata application names
can differ from registered customer-app names. The SDK's session detail query
requires app ID and exact app name. Dropping that name from the key can merge two
distinct dashboard buckets before collecting their session actions, undercounting
or misattributing evidence.

Fix: use `(app_id, exact_app_name, session_id)` for discovery/source identity. Keep
registered-app ID and observed dashboard bucket identities separate; app-ID filters
can intentionally include all matching name buckets. Only create a cross-bucket
association when explicit evidence permits it. Add a case with the same app ID and
session ID under two different application names and assert both detail reads and
unmerged source provenance. Also test a registered app name differing from both.

### H2. A single unspecified action field can conflate enforcement levels

Plan lines 166–174, 188–205 and chunk 2A group or stratify on `action`, without
specifying whether it is scan enforcement or a detector-row action. Pinned
`ScanResponse.action` and `DetectionServiceResult.action` are distinct fields;
reports can contain several detector rows. A detector row may be benign, have an
absent action, or disagree with scan-level action. The original task even notes a
benign DLP detection alongside a prompt-injection report. This matters directly to
the approved monitor-versus-block analysis.

Fix: persist scan-level verdict/action, detector-level verdict/action, and current
profile configuration separately, each with source provenance. Define the grouping
and sampling action as observed scan-level enforcement when resolved; otherwise
explicitly unknown. Never promote a detector-row action or current configuration to
the historical scan action. Surface contradictions as conflicts, preserve the
reported values, and suppress definitive enforcement recommendations. Only
violation detector rows belong to violation counts; benign/unknown rows can remain
supporting context. Add mixed benign/malicious detector reports, mismatched actions,
missing aggregate results and one scan with several detector actions.

### M1. Finish command grammar and scope every limit

Plan lines 100–131 and 207–213 leave `--all` unspecified and do not say which of the
new budgets apply to query/results/reports. `--limit`/`--offset` are redefined as
normalized-view pagination, but defaults, ordering, filtering-before-slicing and
requested-ID caps are not fixed. `--max-pages` being “per source” is ambiguous when
one source fans out into thousands of session walks. The event ceiling has no
explicit CLI control despite the surrounding “configurable” language.

Fix: add a command/option applicability table. Define query single-page versus
`--all` behavior, stable sort keys, filter -> offset -> limit order, and metadata
envelope including incomplete discovery. Specify whether the page cap is global,
per endpoint family or per session; document an aggregate fan-out ceiling. State
repeatable-ID count and byte limits, scalar option duplication behavior, and
whether the 10000-event ceiling is fixed or configurable. Require these checks
before tenant reads and add parse/plan tests for each command.

### M2. Reserve resources for detail enrichment

Plan lines 188–197 and 207–213 put a global budget around candidate enumeration but
do not allocate it between discovery, result resolution and actual detailed report
reads. A default run can spend its full 120 seconds walking sessions and never
retrieve the snippets that are the feature's main purpose. Hard limits prevent
runaway work but do not prevent starvation.

Fix: give discovery an explicit sub-budget. A workable initial strategy is at most
60 seconds and 600 outbound attempts for discovery/configuration, with remaining
global time/attempts reserved for result resolution and selected reports. Unused
discovery allowance carries forward; the global deadline remains authoritative.
Use bounded per-application/session scheduling so the first high-volume application
does not consume the whole candidate budget. Record early-discovery termination as
partial, not a statistically representative sample. Test a slow/large first
application plus a second application with a violation: the run must make useful
detail-read progress or explicitly document that no eligible IDs were resolved.
Make deadline accounting include OAuth and request replay through the existing
transports. Rendering/publication are outside the collection timer.

### M3. A synthetic join is a compatibility proof, not live semantic qualification

Chunk 0A, lines 223–232, asks a synthetic fixture to “prove” the join between
`scan_sub_req_id` and `req_id`. A fixture authored to make them equal cannot
establish that the real service uses the same identity for every sync/async path.
The plan already acknowledges this risk but the gate should make its limits
explicit.

Fix: document each supported join rule and its basis: pinned type contract,
existing repository correlation behavior, and any user-provided qualification
summary. Match exact supplied IDs only; do not infer equivalence where contracts
do not establish it. Unsupported/ambiguous variants become unresolved evidence.
Label the offline chain a “pinned SDK compatibility test.” Keep real-service
qualification user-run and read-only, and list it as a remaining data limit rather
than requiring the agent to obtain production evidence.

### M4. Define canonical evidence digest and rendering version behavior

Plan lines 146–174, 244–251 and 343–362 bind annotations to an evidence digest but
do not define what bytes are hashed or how offline re-redaction affects it. If
generated HTML or reviewer metadata enters that digest, the round trip becomes
circular or stale after legitimate annotation. If a newer ruleset silently replaces
the old rules, a saved snapshot is not fully reproducible.

Fix: hash a deterministic canonical representation of the immutable saved evidence
only, excluding the digest field itself and all review/render metadata. Verify the
input digest before applying additional offline safety projection. Evidence stores
the generating rules/redaction version; explicitly reject unsupported versions or
make an upgraded rerender conspicuous with original and new versions. Keep original
collection evidence unchanged; store annotations/render metadata separately. Tests
must cover whitespace/property-order changes, tampered evidence, new redaction
behavior, and repeated rendering with the same evidence/rules producing equivalent
analysis except declared rendering-time fields.

### M5. Close durability and unsupported-filesystem behavior for bundles

Chunk 4B, lines 330–339, correctly chooses no-replace directory publication, but
“write/fsync all three artifacts” does not explicitly fsync the staging directory
whose entries are about to be renamed. It also needs a refusal path for filesystems
that do not support the atomic primitive. Parent-directory sync alone is not the
entire durability sequence.

Fix: sync each file, sync the populated staging directory, publish with
`rustix::fs::renameat_with(..., RenameFlags::NOREPLACE)`, then sync the parent.
The pinned rustix 1.1.5 supports this on Linux and Apple (Apple maps NOREPLACE to
RENAME_EXCL). If unsupported, refuse safely; do not fall back to overwrite-capable
rename. Add failure injection for staging-directory sync, unsupported rename flags,
and parent sync after successful publication. State the artifact remains saved
after publication even when durability confirmation fails.

### M6. Establish contracts at chunk boundaries without premature documentation drift

Chunk 0B proposes updating binding DESIGN sections before code exists; chunk 6A
groups parity and documentation migration near the end. The introduction promises
contract updates and tests with every observable change, so execution needs to
follow that promise rather than interpreting phase 6 as permission to postpone them.

Fix: phase 0B should prepare proposed contracts in a clearly marked research/spec
document. Promote each implemented behavior into DESIGN and COMPATIBILITY with
its executable and dNN parity tests in that same chunk. Phase 6A then reconciles
cross-cutting references and generated help. This preserves an honest, passing
repository at reviewable checkpoints instead of a long run of temporarily false
contract claims.

## Positive feasibility findings

- The actual Cargo registry `scm-sdk-0.2.4` contains both Scanner query methods and
  their 1–5-ID constraints. `ScanResponse.report_id` provides a typed report reference.
- Pinned detailed reports contain optional scan/request/session/transaction IDs,
  per-detector verdict/action and all four typed snippet families.
- Native no-replace directory publication is available through the existing safe
  rustix dependency on Linux and macOS; no new unsafe block is needed.
- Transport wrapping already centralizes SDK HTTP sends, making shared atomic
  attempt/concurrency admission feasible without bypassing the SDK or credential
  plane rules. Such instrumentation must be invocation-scoped to avoid changing
  unrelated report commands.
- The proposed file-based agent review keeps API observations immutable and does
  not authorize automated external agent execution or remediation.

## Review limits

This review read local source only and did not execute the test suite because the
change is a future implementation plan. The prior host baseline is not evidence
that this feature exists. No actual dashboard-to-report workflow is newly qualified
by this review. The root agent should revise the plan, retain this initial score
and findings, then request an independent re-score of the revision.

## Follow-up review of revised plan

Reviewed the revised plan on 2026-11-02 after the original findings were addressed.
The initial score and findings above are retained as review history. References in
the initial findings apply to the initial 435-line revision, not the revised file.

| Dimension | Revised score / 10 |
| --- | ---: |
| Scope fidelity | 9 |
| SDK/implementation feasibility | 9 |
| Correctness and privacy | 9 |
| Delivery sequencing | 9 |
| Validation | 9 |
| **Overall** | **9** |

**Decision: ready for implementation.** The planning quality gate is satisfied.
There are no unresolved critical or high-priority plan findings. This is a plan
approval, not an assertion that the feature has been implemented or live-qualified.

Verified resolutions:

- H1: the exact `(app_id, app_name, session_id)` discovery identity and name-bucket
  isolation now appear in the contract and explicit synthetic acceptance tests.
- H2: scan enforcement and detector actions/verdicts are independent, unknowns and
  conflicts remain visible, and benign detector rows are excluded from violation
  counts. Sampling/grouping use observed scan enforcement only.
- M1: option applicability, per-family aggregate page bounds, event limits,
  query ordering/filtering/slicing, direct ID caps and partial-query exits are now
  explicit. Unknown/failed lookup does not become benign.
- M2: 40/30/30 discovery/enrichment/detail time and outbound-attempt reservations,
  carry-forward, round-robin traversal, actual HTTP admission and starvation tests
  resolve the resource-allocation omission. Optional charts cannot starve details.
- M3: the synthetic chain is correctly described as pinned-SDK compatibility proof;
  live equivalence remains unqualified, and unsupported joins stay unresolved.
- M4: canonical evidence hashing, immutable source evidence, version replay,
  additional privacy projection and digest/annotation binding are defined.
- M5: staging-directory fsync, safe Linux/macOS no-replace publication,
  unsupported-filesystem refusal and post-publication durability warnings are covered.
- M6: proposed contracts stay in research initially; each observable implementation
  chunk promotes its binding contract and executable/parity coverage together.

The follow-up also caught a new privacy interaction: blindly copying immutable
source evidence into a rerendered bundle could republish forged sensitive content,
despite a valid recomputed digest and safe HTML projection. The root revised the
plan before this final score. It now explicitly treats digests as integrity rather
than authenticity, requires privacy approval before unchanged source reuse, refuses
unsafe offline bundle output before publication, and permits safe HTML-only
rendering without changing the source. An adversarial recomputed-digest test is
required. This finding is resolved at the planning level.

Implementation checks still matter: canonical parsers should reject duplicate JSON
keys, outbound-attempt admission must cover OAuth/retries without deadlocking token
refresh, and stage deadlines must cancel outstanding work rather than only stop
scheduling. These are bounded engineering obligations covered by the revised
validation work, not reasons to extend the scope interview. No feature tests or live
calls were run during either plan review.
