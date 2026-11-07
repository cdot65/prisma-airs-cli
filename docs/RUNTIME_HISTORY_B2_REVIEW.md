# B2 independent review — automatic received metadata

Reviewer: independent Codex `/root/b2_review`. Final independent score: **9/10 — accepted**. No unresolved material B2 blocker.

## Early findings and corrections

The reviewer identified concrete gaps before final qualification:

- Add the approved `--history-dir` control and conflict with `--no-history`, with
  pure usage validation and repeated-flag checks before tenant access.
- Retain complete/completed statuses and supported empty nested UUID context.
  Preserve report verdict aliases, known service aliases and administrator actions;
  unknown/missing/conflicting actions cannot become allow. Known benign/error verdicts and
  unknown verdicts are preserved independently of administrator block actions. Oversized/truncated metadata cannot
  claim benign or complete action context.
- Key receipts by bulk job/item or synchronous invocation/item, preserving variants
  across polling/resume rather than hashing source/UUID or resetting a batch ordinal.
  Unsupported item association gets a separate unlinked observation ID. Batch report
  IDs are contextual provenance and cannot be assumed to belong to each item.
- Known scanner endpoint receipts need exact known full-event endpoint support.
  Legacy operator-bound unknown endpoints cannot prove named binding compatibility.
  This conservative rule avoids circular promotion based on other promoted receipts;
  later conflicting full endpoint facts invalidate promotion in either arrival order.
- Secondary SQLite work must follow the essential bulk recovery checkpoint. The
  adapter now captures bounded normalized metadata in memory, then flushes only
  after C3/poll state is durable. CSV failure after that checkpoint can retain the
  metadata; a failed checkpoint cannot create history. Existing CSV error/output
  semantics and remote calls remain unchanged.

The reviewer questioned the capture cap for large logical batches. Source review
confirmed the existing planner limits each SDK submission to20 objects and flushes
between calls. A45-item logical batch regression proves all three SDK chunks and
all45 receipts survive; explicit oversized-response limits still warn safely.

Additional tests exposed public endpoint handling: generic personal-identifier
projection was withholding admitted loopback IP endpoints, discarding intact named
binding context. Public endpoints now retain their exact admitted namespace while
other reference labels are projected, with credential/userinfo/query checks kept.
Canonical trailing-slash handling matches the scanner configuration contract.
A removed-after-admission database is also refused rather than recreated.

## Qualification status

Focused results:25 history foundation/repository/normalization tests and7 real CLI
retention tests pass. They cover actualINT130/TERM143, durable receipt progress,
resume after same-app credential rotation, no resubmission, repeat-resume dedup,
whole-file payload/key/snippet absence, one quiet-safe warning, cumulative250ms
contention batches, explicit opt-out/path usage, and failed-C3 no-history creation.
Existing scan84 and bulk64 tests plus legacy import10 also pass. D348's paired
frozen TypeScript difference passes; its initial test incorrectly expected TS JSON
output despite existing D5 and was corrected to compare the shared pretty format.
No production behavior was changed to accommodate that fixture mistake.

Final required checks and all-three-platform execution pass; exact inputs and
receipt fingerprints are in RUNTIME_HISTORY_B2_QUALIFICATION.md. No live APIs, real credentials, private
validation bundles, SDK edits, commits, tags or publishing were involved.


The initial full run exposed a harness state-guard fixture that did not provide
its newly required data root; giving that fixture an isolated data root preserves
its state-fallback assertions, while dedicated tests pin automatic data-root safety.
Native Mac parallel tests hit the fixed write deadline under independent fsync
load; exact-source sequential tests passed all193 selected cases. Both failures
and serial settings are retained. ARM QEMU exposed repeated per-record receipt
reconciliation work and an outer wall-clock assertion that conflated emulation/
normalization with contention. Reconciliation now runs once per distinct resolved
UUID per transaction, with an unknown-new-fields regression. The outer test guards
against long retries/backoff; direct250ms policy assertions remain unchanged. This
product change requires a fresh full/target snapshot; earlier receipts do not
qualify the new implementation.


The final harness audit found a flag-shaped-value edge: profile/response values
`--no-history` were mistaken for the switch by the test runner. Value-aware parsing
and both pure/CLI regressions preserve automatic retention and all path guards.
A late-echo test also proves payload-registry overflow refuses archive creation and
warns once rather than persisting text that could not be registered. The inherited
D120 test serializes/checkpoints ten MiB over multiple SDK calls; its outer test
harness budget is120seconds for emulation, with every original chunk/body assertion
and the production60second request policy unchanged. Acceptance3 reopens the full
checks and all target suites after these shared harness/test changes.

## Final independent assessment

Independent Codex `/root/b2_review` accepted B2 at9/10 after verifying acceptance3
manifest6be112846f0bd2ae81fbe9fe2ebd668818c9239e02635900c87e7a724523d95f,
all457 archived members/hashes and no non-documentation source drift. The reviewer
verified all four qualification receipt hashes and both static Linux CLI hashes,
parsed3788/0/5 across97 required workspace groups with no parity skips, and verified
25 foundation plus170 integration passes on each Rust1.89 target, with one existing
ignored helper. Independent reruns of25 history,7 retention and10 import tests pass.
The reviewer accepted the documented emulation/parallel-load limitations and final
harness/overflow/reconciliation corrections. No unresolved material B2 blockers.
B3 may proceed; later sync, transfer, rendering, scale and release gates remain.
