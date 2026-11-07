# B1 durability qualification

This qualifies the implemented8.2.0-dev.1 storage foundation, not a finished8.2
release. B2 automatic retention, B3 sync, B4 transfer/pruning, B5 historical reports
and B6 end-to-end release qualification remain separate gates. No live API requests
or real credentials were used; all inputs are synthetic or loopback fixtures.

## Executable snapshot and checks

The acceptance2 source manifest contains451 public source/test/documentation files:
`13a0921aee903f5181310449f1e6ebb457de2a67ce88bc55cedd80550ded7c79`.
Root executable inputs matched the built snapshot. Later documentation annotations
are not represented as having been built or tested in that frozen source copy.

Required checks passed:

| Check | Result |
| --- | --- |
| cargo fmt --all --check | pass |
| cargo clippy --workspace --all-targets -- -D warnings | pass |
| AIRS_REQUIRE_PARITY=1 cargo test --workspace |3769 passed,0 failed,5 ignored,96 groups,0 parity skips |
| Rust1.89 workspace/all-targets check | pass |
| Documentation site build | pass |

Node24 supplied the frozen TypeScript7.3.0 reference for the required parity run.
The existing live tier was not enabled. The process runner kept child checks at
umask022 while storing orchestrator logs privately.

## Target execution

All three shipped target families compiled and executed the new bundled SQLite
serialize integration with Rust1.89. Each ran15 foundation tests plus5 local
inspection and10 legacy-import tests. Linux amd64 uses a native static musl ELF;
Linux arm64 uses qemu-aarch64-static11.1.2 and registered binfmt for CLI children.
macOS arm64 runs natively on worf (Apple M5 Pro). Debug qualification includes the
actual SIGINT/SIGTERM committed-batch tests; it is not a release-package claim.

| Target | Foundation | Inspection/import | Execution |
| --- | --- | --- | --- |
| x86_64-unknown-linux-musl |15/15 |15/15 | native |
| aarch64-unknown-linux-musl |15/15 |15/15 | QEMU |
| aarch64-apple-darwin |15/15 |15/15 | native |

The initial macOS test build failed because mkfifoat is not exposed on Apple;
acceptance2 uses a portable mkfifo fixture creator with unchanged admission
assertions. The earlier Linux attempt's source guard correctly refused acceptance
after that edit; its passing individual tests are not reused as a final receipt.
Initial logs remain available under .local/dev/runtime-history/b1/.

## Test-only qualification delta

After the full workspace run, the existing refusal test added explicit schema0 and
foreign application_id header cases. Production code and dependency/build inputs
were unchanged. The database.rs prefix before its cfg(test) test module is
byte-identical (SHA256 `1c82d463071efeff8a58e43fe54c64e831b05d08d102a19890364d53cc47bd0c`). The independent
reviewer accepted rerunning the complete affected foundation suite on all three
Rust1.89 targets, plus formatting/Clippy/MSRV, rather than another unrelated full
workspace run. All these fresh checks pass. The earlier full run is not claimed
to have executed the additional assertions.

Final test-source SHA256: `1bc8763c1b2b1920144d4d1a6baeb9a939e2538ea2f62ec9902997389635ec84`.
Delta source manifest SHA256: `03448087ae676a689cb0e6e4a370100ef12835e7e8913b1136f7b527898d8ec0`.
The exact test diff and before/after fingerprints are retained with the receipts.

## Covered durability boundaries

Tests exercise private atomic initialization, missing read-only opens, unchanged
corrupt/unsupported/newer/older/cloud/journal refusal, FIFO nonblocking admission,
symlink/mode/hard-link and replaced lock identity checks, cumulative contention,
SQLite deadline/cancellation, disk-full transaction rollback, whole-live-database
secure-delete canary absence, coherent reads and actual committed-batch signals.
A genuine crashed subprocess leaves a hot journal: normal readers/writers refuse
unchanged, and explicit copy-verified recovery restores the pretransaction state
while preserving immutable raw backup bytes. Superjournal and wrong-cloud recovery
are refused before live modification. Compaction fault cases preserve either the
verified old file or verified published copy and report uncertain postpublication
durability honestly.

Schema1 is initial; no existing migration path is claimed. Future schema upgrades
need their own concrete transactional migration and verified backup/failure tests.
The million-row performance observations remain B0 feasibility evidence; B6 must
repeat full product/report/compaction/contention qualification. B1 does not claim
new million-row product results or complete8.2 release packages.

## Raw receipts

- .local/dev/runtime-history/b1/acceptance2/check-results.json: SHA256 `8ce98c2f22ed6c3fe3c4637a33fb57df3553878c70276d8c3f73461f801897c0`.
- .local/dev/runtime-history/b1/acceptance2/test-summary.json: SHA256 `bcef0b347c9475c99a076f92ce1cf56a14964dfa286ca53bf841643787b7acac`.
- .local/dev/runtime-history/b1/acceptance2/linux-receipt.json: SHA256 `2656f2aa1ae5a12ee1822348a7b858420023d0d4794523ac004bd7c204b1ed1b`.
- .local/dev/runtime-history/b1/refusal-delta/delta.json: SHA256 `a3eebe93ce6cedc6816b06cd0ec6f75dbd0c42d13e80449a04f04ecb242d4f86`.
- .local/dev/runtime-history/b1/refusal-delta/linux-platform-results.json: SHA256 `030f26ff97770efea0fbe5c92ede2f5e3108d1a6347e5535ba23ce0cf0710232`.
- .local/dev/runtime-history/b1/refusal-delta/macos-receipt.json: SHA256 `e9367d0ab3ca5ce9e6f6354c3e97e3b934ed1d00c3151682f1777ecb5aac4d7b`.
- .local/dev/runtime-history/b1/refusal-delta/check-results.json: SHA256 `983d92a53da632830ff3717c693d53c231401ee57b31618a1d9830b1d3eb3e1e`.

Native macOS Rust1.89 release compilation passed (492s). The resulting8.2.0-dev.1
binary has no debug signal-hook variable string. This is an additional compile/
hook-exclusion check, not signing/notarization or complete release qualification.
Release binary SHA256: `908d09f369864f4cdb243763fd8851243a16cfac6e862efa3c3c19d05c21c8ad`.
Native Mac receipt SHA256: `16c7fda7869da64135393636787703271d4eb1b6c8e6cd9f6117ae0be2a08451`;
its source manifest matches acceptance2 exactly.
