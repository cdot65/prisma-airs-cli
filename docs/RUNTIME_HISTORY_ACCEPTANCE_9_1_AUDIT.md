# Runtime history: current 9.1 acceptance audit

Status: **B3 accepted 9.2/10; B4–B6 remain unfinished**. The user raised the active
completion threshold to **9.1/10 or higher** on 2026-11-04. AGENTS.md records it.
Historical 9/10 phase assessments retain their original scores and scope.

## Latest reconciliation — 2026-11-04

The user authorized continued isolated work with independent reviewer agents.
B3 subsequently completed acceptance5 and an independent 9.2/10 assessment;
the failed r1/r2 pilot results below remain historical, superseded evidence.
Our read-only auditor verified the 492-file acceptance5 manifest/archive, five
receipt hashes, three retained CLI hashes and the recorded default workspace
result of 3,833 passed, zero failures, five existing ignored tests and zero parity
skips. Full proof and platform limits remain in
[RUNTIME_HISTORY_B3_QUALIFICATION.md](RUNTIME_HISTORY_B3_QUALIFICATION.md).

The process-local history test gate and privacy-marker schema correction resolve
the earlier timing/attribution blockers without changing the production 250ms
retention policy. These fixes were authored and qualified by the other sessions.

Independent reviewer `/root/review_harness` accepted the isolated pre-exec helper
below at **9.2/10**, found no material safety/correctness defect and reran all 257
harness tests successfully. Its exact reviewed runner SHA matches the integrated
shared file. TESTING §5.1 now documents the policy. This score is scoped to the
helper, not B4–B6 or whole-project release readiness.

A fresh isolated run of accepted B3 plus the integrated helper passes **3,835
tests, zero failures, five existing ignored tests across 98 groups**, with parity
required and zero skips. Formatting, workspace/all-target Clippy with warnings
denied, Rust 1.89 workspace/all-target checking and the documentation production
build also pass. The 531-file public-source manifest SHA256 is
`73387f9fcb70aa883a91a061b8528e590667faba37d8e60e0b5230648a4ed99f`;
logs and `b3-helper-test-summary.json` remain under
`/var/tmp/airs-9p1-9_jsua1s/`. This run excludes unfinished B4 source.

B4 implementation is underway in the existing session. This team is preparing
independent transfer/prune probes and grounded code reviews in isolation; it has
not taken over or claimed that session's implementation. B4 must independently
reach at least 9.1 before B5, and the remaining phase gates still apply.

This audit has not implemented or taken ownership of the B3 changes being made
by other sessions. Two other CLI sessions were open in the shared checkout and
source edits were observed during verification. Public source was captured in an
isolated detached worktree; no private validation bundles, tenant files or SDK
checkout were copied. All executable checks use synthetic/loopback fixtures.

## Source and evidence boundary

The isolated checkout is `/var/tmp/airs-9p1-9_jsua1s/source`, based on main
`2d2b460` plus the working-tree public source. The initial r1/r2 snapshots each
contain 529 allowlisted files; the later B3/helper snapshot contains 531. The
pinned TypeScript reference is shared read-only; tests
use Node 24.22.2 and require parity. No live API qualification, release, signing,
commit or push was performed by this audit.

| Snapshot | Source-manifest SHA256 | Outcome |
| --- | --- | --- |
| r1 | `a0e98518a3d6b353ce1428fe9505b3939e1bd51fe410da06d3a11c233987927f` | Focused 42 tests pass; full library and Clippy fail |
| r2 | `5aa3e3e34d371ee800f38b0b5b57686ba2ddc5922dba9d50a5a6f16e2e6867f7` | Format/Clippy/MSRV/docs pass; default library scheduling has four failures |

r1 source is preserved in the parent directory's `snapshot-r1/source.tar.gz`,
with its manifest. Logs for both attempts remain there. r2 checks do not certify
later edits in the shared checkout, including subsequent repository privacy
changes. A final reviewed revision needs a new coherent source fingerprint.

## Required-check findings

1. r1 Clippy rejects the eight-element stored-work tuple at
   `src/history/work.rs:191` under `-D warnings`. The active implementation session
   subsequently factored its type; r2 Clippy passes without a lint waiver.
2. r1's promoted-receipt/report-ownership fixture expects `promoted` but observes
   `conflicting`. The active session replaced that fixture; r2's revised cache
   demotion test passes. The r1 library run records 946 passed, one failed and one
   ignored; it is not a full workspace pass.
3. r2 default library scheduling records 943 passed, four failed and one ignored:
   `named_receipts_require_known_endpoint_support_in_both_orders_and_invalidate_on_conflict`
   and `rotation_keeps_revisions_and_batch_report_context_is_not_item_ownership`
   exceed the archive writer deadline;
   `completion_contention_preserves_received_data_and_warns_once_without_zero_loss_claim`
   misses its expected completion-warning text; and
   `unknown_item_receipts_do_not_coalesce_across_flushes_and_capture_is_bounded`
   observes three receipts where it expects six. A separate serial diagnostic
   passes all 40 history unit tests. This establishes scheduling sensitivity,
   not a passing default workspace check or permission to relax the production
   250ms retention policy.

r2 formatting, workspace/all-target Clippy with warnings denied, Rust 1.89
workspace/all-target checking and the documentation production build pass.
Separate integration/parity verification completes 91 groups: 2,705 passed,
one failed, four ignored and no parity skips. The additional failure is
`sandbox_runner::model_security_runs_check_the_markers_on_stderr_and_the_debug_log`:
the fixture script fails to execute with `Text file busy` (`ETXTBSY`). These
counts describe separate runs and must not be added as unique passing tests.

## Prepared harness correction

An isolated patch retries only `ETXTBSY` failures before a child starts, for at
most one second within the existing run timeout. Other spawn errors still fail
immediately; neither an executing CLI nor an API call is replayed. A Linux test
holds a real writable fixture script open, observes the initial `ETXTBSY`, closes
the writer and verifies that the retry starts exactly one successful child.
Another test pins immediate failure for permission errors and an expired retry
deadline.

The complete harness suite passes 257 tests, zero failures, zero ignored and no
parity skips after this fix. Workspace/all-target Clippy and the Rust 1.89
harness/all-target check also pass. The initial new unit-test compile attempt
used the wrong rustix constant spelling; that attempt remains in
`spawn-fix.log`, and the corrected passing attempts have separate logs.

Patch: `/var/tmp/airs-9p1-9_jsua1s/spawn-ready.patch`. After the scoped independent
9.2 assessment and a clean apply check, it was integrated into the shared checkout.
Fixed runner SHA256:
`f883d1b66b2b7ac6e522bbc9d1c928fb418a0e4dbb5d13095a1f13df02382328`.
Machine-readable results are in the parent directory's `audit-results.json`.
This patch does not itself fix the four separate history timing failures;
B3 acceptance5 resolves those separately as described above.

## Remaining acceptance work

- Preserve the timing-sensitive fixture fixes, production deadlines, honest
  warnings and meaningful concurrency tests in subsequent phases. Keep failed
  attempts and documented execution conditions with final receipts.
- Proceed through the already approved B4 portability/pruning, B5 historical
  reports and B6 scale/scheduling/build qualification gates. These phases remain
  unfinished; passing B3 tests cannot substitute for their implementation.
- Continue isolated implementation/integration and independent review as the
  user authorized. Do not overwrite another session's source or attribute its
  work to this audit.

Owner Validation and user-run live-service qualification remain distinct from
technical review and synthetic tests. No unattended maintenance was installed.
