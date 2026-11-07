# B1 independent review — SQLite durability and repository

Reviewer: independent Codex `/root/phase6_review`. Final score: **9/10 — accepted**.
No unresolved material B1 blockers. The earlier findings and qualifications below
are preserved. Claude's plan score did not certify this implementation.

## Review findings and corrections

Early source review identified filesystem, interruption and identity issues.
The corrections and executable evidence are retained rather than replacing that
history with a claim that the first implementation passed:

- Verify lock pathname/inode after waiting and before database access. Admit files
  with regular-file prechecks and nonblocking no-follow opens, including FIFO
  subprocess tests. Build initial SQLite bytes in memory and serialize through an
  owned descriptor before no-clobber publication, avoiding unverified staging writes.
- Use an exclusive maintenance gate for every mutating SQLite open. This reviewed
  B1 refinement supersedes B0's shared retention gate while retaining the cumulative
  250ms retention deadline; it closes the cooperative journal/precheck race. Never
  hold a data gate or transaction over network work. Full read-only maintenance
  uses an existing READONLY archive and cannot initialize missing data.
- Capture cancellation/deadline in gate waits and SQLite busy/progress handlers.
  Actual SIGINT/SIGTERM tests after a committed import batch report possible
  committed progress and resume safely, rather than claiming no data was written.
- Distinguish privacy-withheld labels from supported identities. Profile and
  population observations with changed/already-withheld labels are collection-scoped
  unlinked facts, never hashes of private labels used as cross-run associations.
  Retain application inventory with no scan events; unsupported application identities
  have null join columns. Coverage retains selector context and cannot convert a
  changed/masked selector into application-scoped completeness. Legacy complete
  sources mean observed, not traversed-as-of.
- Preserve run links for repeated A/B/A snapshots and out-of-order observation
  times. Keep positive detector hints separate from report context, retaining
  unknown-direction report rows when subsuming only their comparison contribution.
  Error plus allow is unknown/conflict. Reconciliation pages receipts, applies all
  supplied candidate predicates before LIMIT2, and shares one16MiB rebuild budget.
- Recovery keeps immutable raw database/journal copies, verified by readback SHA
  and size, plus versioned private manifests. Recover and validate a second working
  pair before rechecking original bytes/inodes and locks immediately before native
  live SQLite rollback. Reject superjournal references before SQLite opens copies.
  A genuine process-crash hot-journal test proves refusal and explicit restoration.
  Compaction tests cover copy/publication faults and uncertain postreplacement
  directory durability without silently rolling back to an older file.

The reviewer agreed moving the explicit legacy `import-bundle` adapter from B4
into B1 makes this foundation independently usable/testable. B4 logical transfer,
pruning and lifecycle work remain separate. The reviewer also accepted initial
schema1-only versioned admission/refusal: there is no published predecessor and no
supported upgrade path yet. Do not add an unused migration engine or claim future
migration qualification. The first real schema2 change must supply its concrete
transactional migration and verified backup/failure gates before shipping.

## Qualification record

Current focused results:15 history foundation tests,5 inspection tests,10 import
regressions,3 required paired TypeScript differences and43 harness tests pass.
The new zero-event inventory test initially had an invalid sample ledger pointing
to removed reports; fixing that synthetic fixture allowed the ingestion behavior
to be tested. No production validation was weakened.

Root full workspace/required parity, Rust1.89, docs build and Linux amd64/arm64 musl
plus native macOS qualification now pass. Complete scope, source fingerprints,
test-only refusal delta and raw receipt checksums are recorded in
RUNTIME_HISTORY_B1_QUALIFICATION.md. Final independent numeric score is9/10, accepted.
Raw receipts are in
`.local/dev/runtime-history/b1/`. All evidence is synthetic/loopback. There have
been no live API requests, credential reads, commits, tags or publication.


The initial macOS build exposed a test-only portability failure: rustix does not
expose mkfifoat on Apple targets. The FIFO test now creates the same private fixture
using the portable mkfifo utility; all admission assertions remain. Initial logs
are retained, and acceptance2 reruns use the corrected source. The older-schema
error also now says unsupported/use-compatible-version rather than implying an
existing migration command.


## Final independent assessment —9/10, accepted

The reviewer verified every archived member/hash in both451-file snapshots, the
unchanged production prefix and precise test-only delta, and the current non-doc
source match. It parsed the complete required workspace receipt (96 groups,
3769 passed,0 failed,5 ignored,0 parity skips), verified all documented receipt
hashes and Linux static-binary hashes, and confirmed Rust1.89 execution of all
three15+5+10 target suites plus fresh15-case foundation deltas. It independently
reran the final15 foundation tests and earlier5 inspection/10 import tests.
The native Mac release compile/source/hook-exclusion chain is consistent, without
claiming signing or package qualification. Early findings are resolved.

B2 may proceed. It must separately qualify actual named receipt binding/endpoint
provenance; legacy imports' operator-bound unknown endpoint is not that proof.
B3 sync, B4 logical transfer/pruning, B5 historical UI and B6 performance/release
qualification remain required. No live/private data or credentials were used.
