# Runtime credentials/history implementation phase record

Started 2026-11-04 after Claude Code plan scores 7, 8, then 9/10. Full assessments
are in RUNTIME_HISTORY_CLAUDE_REVIEW.md; independent Codex plan assessments and
revision hashes are in RUNTIME_HISTORY_PLAN_REVIEW.md. Plan acceptance is not
implementation acceptance. Every phase requires its own independent Codex >=9.

## A0 — Capability and command contract

Status: complete, independently accepted **9/10**. Contract:
RUNTIME_CREDENTIAL_CONTRACT.md. Review: RUNTIME_CREDENTIAL_A0_REVIEW.md, preserving
initial 8/10, corrected legacy retry-default attribution, and the test-launcher
umask correction. Final fmt/Clippy/required-parity workspace checks pass:
3697 passed, 0 failed, 5 ignored, 90 groups and no parity skips. SDK-focused checks
pass 4/4. No CLI behavior is represented as integrated before phase A1.

## A1 — Named configuration and guarded routing

Status: complete, independently accepted **9/10**. Required format, Clippy and
workspace checks pass: **3727 passed, 0 failed, 5 ignored, 93 groups, no parity skips**. Review: RUNTIME_CREDENTIAL_A1_REVIEW.md.
Implemented strict named tenant scanner settings, explicit private input/binding,
durable no-clobber rollback snapshots, masked read/doctor views, direct selectors,
application-bound discovery/report routing, global result/report budgets and
version-2 named bulk references with guarded resume and rotation. DESIGN,
COMPATIBILITY, setup docs and generated CLI references updated.

Independent review found and drove fixes for repeated scalar options, missing
native-old-reader evidence, token wire coverage, credential coincidences in error/
doctor/bulk output, and canonical endpoint expansion. Initial score8 and final9
assessments are preserved. Eleven routing and nine local scanner tests pass,
including all-format privacy checks. Last full-suite attempt found an unrelated
legacy gateway error changed by overbroad final filtering; the final filter now
applies to named-namespace/scanner-management invocations and the gateway
regression passes. The final required-parity run passes after that correction.
Earlier failing logs remain under .local/dev/runtime-history; no check is waived.

Frozen pre-A1 native qualification passed against a private synthetic-only store:
flat read succeeds (0), explicit binding saves exact-byte old snapshot, frozen RC3
refuses named tables (1) without writes, explicit restore returns exact original
bytes, then frozen RC3 read succeeds (0) without writes. No HTTP calls. Baseline
SHA256: `0856de0c6611239849a8b70f95f03af91940975865849c92334a9c4a3e08ce94`.
Local machine-readable receipt: `.local/dev/runtime-history/a1-native-downgrade.json`.

## B0 prerequisite carried from Claude Review 3

Before B0 starts, incorporate C1 and associated review recommendations into its
storage contract: whole-archive export/statistics/prune preview cannot use an
unbounded reader transaction or an incoherent mutable-row cutoff. Choose an
explicit maintenance operation with writer lease/exclusive gate and bounded
chunks/deadline, or a verified private snapshot. Prove consistency for chunked
report views; name write-hold targets, lock order, missing-gate refusal, compaction
temporary storage and memory bounds. Require 30/90-day million-scan reports to
complete within the read deadline on documented hardware. These are prerequisites,
not waived implementation tests. This carry was recorded before B0 started.

## A2 — Evidence recovery and collection diagnostics

Status: complete, independently accepted **9/10**, preserving initial8 findings
and remediation in RUNTIME_EVIDENCE_A2_REVIEW.md. Typed positive management action
metadata retains unknown direction/enforcement and raw provenance. Session-level
observations stay separate; positive queues retain app fairness. Per-client
diagnostics distinguish returned, pending, rejected, missing, empty and failed
reads from actual transport sends. Summary aggregates preserve exact populations/
windows, reject scales and contradictory counts, and privacy projection cannot
create joins or semantic labels. Comparison-only direction subsumption prevents
duplicate counting without deleting raw hints.

Required format/Clippy/workspace checks pass: **3735 passed,0 failed,5 ignored,
93 groups,no parity skips**. Final receipts: .local/dev/runtime-history/
a2-acceptance-results.json and a2-test-summary.json.

## A3 — 8.1 release-candidate qualification

Status: complete, independently accepted **9/10** in RUNTIME_RC4_A3_REVIEW.md,
preserving initial8 readiness assessment. All three8.1.0-rc.4 platform archives
are qualified at target/dev-builds/signal-brief-8.1.0-rc.4/packages and on worf.
Linux arm64 execution uses QEMU;62 selected tests target the frozen packaged ELF.
Mac native selected tests62/62, native/extracted/final combined archive workflows
and ad hoc signature verification pass. All archives/checksums/layout/aliases and
source provenance independently verified. Source manifestSHA:
3f890dacef9b761a4ad45cf4bc5e3ac54da754fe24f087830e27acc2c861047e.
Final BUILD-INFO SHA534c9dcf4d3869876f8da463e30991ffae2610b1089d9cc3b65f9e34161ea0f6
is identical on enterprise and worf. Required checks3735/0/5,93 groups,no parity
skips; Rust1.89 and docs build pass. Frozen RC3 new-format/new-state refusal,
no-send removed binding, rotation/no-resubmit and exact rollback verified.
No commit, tag, publishing or workflow change. Service qualification remains user-run.

## B0 — History contract and bundled SQLite feasibility

Status: complete, independently accepted **9/10**. Initial8/10 and cumulative
contention fix are preserved in RUNTIME_HISTORY_B0_REVIEW.md. Contract:
RUNTIME_HISTORY_STORAGE_CONTRACT.md; measured evidence:
RUNTIME_HISTORY_B0_QUALIFICATION.md. C1 resolved before database code. Isolated
rusqlite0.41.1/bundledSQLite3.54.1 passes Rust1.89 and all three shipped target
executions, including million-row30/90-day reads within2seconds (ARM underQEMU).
Gate-plus-SQLite contention uses one250ms deadline and measures250ms on each
platform. RootRC4 code/dependencies remain unchanged at acceptance.

## B1 — SQLite durability and repository layer

Status: complete, independently accepted **9/10** in RUNTIME_HISTORY_B1_REVIEW.md.
Private archive/repository, local inspection, explicit legacy import and
compaction/recovery are qualified. Required workspace3769/0/5,96groups,no parity
skips; fmt/Clippy/MSRV/docs pass. All three Rust1.89 target families pass15 storage
plus15 inspection/import tests; the final test-only header-refusal delta passes
fresh foundation suites on all three. Source/receipt fingerprints and limitations
are recorded in RUNTIME_HISTORY_B1_QUALIFICATION.md. Schema1 is initial; future
real migrations require their own transactional/backup gates. No automatic
retention, sync or historical report qualification is implied.

## B2 — Automatic scan-result retention

Status: complete, independently accepted **9/10** in RUNTIME_HISTORY_B2_REVIEW.md.
Automatic received metadata retention, opt-out/path controls and one-warning
behavior are qualified. Required workspace3788/0/5 across97 groups, no parity
skips; fmt/Clippy/MSRV/docs pass. All three Rust1.89 target families pass195 selected
tests plus one existing ignored helper. Exact snapshots, initial failures/fixes and
serial test limitations are in RUNTIME_HISTORY_B2_QUALIFICATION.md. No extra HTTP.

## B3 — Resumable sync and explicit enrichment

Status: complete, independently accepted **9.2/10**, meeting the updated >=9.1
threshold in AGENTS.md. RUNTIME_HISTORY_B3_REVIEW.md preserves findings and final
assessment; RUNTIME_HISTORY_B3_QUALIFICATION.md records exact source/receipt/binary
hashes and limitations. Default workspace3,833/0/5 across98 groups, no parity skips;
fmt/Clippy/MSRV/docs pass. All three Rust1.89 target families pass239 selected tests
plus one existing ignored helper. Native AMD64/Mac and isolated emulated ARM64
conditions and earlier failed attempts are disclosed. No live/release/performance
acceptance is implied. Default report behavior stays8.1 untilB5.

## B4 — Portable history and lifecycle

Status: unfinished; CLI-local implementation paused for the user-approved optional
MCP architecture. Existing transfer work and independent findings are preserved
for server-side completion and fresh qualification. B4 has no acceptance score.

## Optional MCP architecture supersedes local delivery

The user selected a separate optional MCP crate/binary in this workspace, with
server-owned storage, jobs and reports. Standalone CLI live reporting remains
available; explicitly selected MCP operations never fall back to live collection.
The new delivery phases are specified in
[RUNTIME_MCP_IMPLEMENTATION_PLAN.md](RUNTIME_MCP_IMPLEMENTATION_PLAN.md), with
plan review in [RUNTIME_MCP_PLAN_REVIEW.md](RUNTIME_MCP_PLAN_REVIEW.md).

A0–B3 scores above remain historical acceptance of their frozen source. B4–B6
delivery obligations move into the new M0–M8 phases; none is accepted by this
planning change. Each outstanding implementation/requalification gate remains
at least9.1, with fresh executable evidence and independent review.
