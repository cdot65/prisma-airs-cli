# B2 automatic metadata retention qualification

This qualifies automatic received-result retention in8.2.0-dev.1. Sync, transfer,
historical reports and release qualification retain their separate B3–B6 gates.
All test data is synthetic or loopback; no live APIs, real credentials or private
validation bundles were accessed. No publication or signing is claimed.

## Frozen inputs and required checks

Acceptance3 contains457 public files, source manifest SHA256
`6be112846f0bd2ae81fbe9fe2ebd668818c9239e02635900c87e7a724523d95f`.
Executable/dependency/test inputs matched the built snapshot after target execution.
Later review/documentation annotations are not represented as built inputs.

| Check | Result |
| --- | --- |
| cargo fmt --all --check | pass |
| cargo clippy --workspace --all-targets -- -D warnings | pass |
| AIRS_REQUIRE_PARITY=1 cargo test --workspace |3788 passed,0 failed,5 ignored,97 groups,0 parity skips |
| Rust1.89 workspace/all-targets check | pass |
| Documentation site build | pass |

The full GNU workspace used default test concurrency and Node24/frozen TypeScript
7.3.0 parity. Child checks use umask022, orchestration receipts are private.

## Shipped target families

Each Rust1.89 debug target ran25 history foundation/repository/normalization tests,
84 scan,64 bulk,5 inspection,10 import and7 retention tests:195 passed and one
existing ignored bulk helper. Linux amd64 musl ran natively; Linux arm64 musl ran
under qemu-aarch64-static11.1.2 with CLI children through registered binfmt. Mac
arm64 ran natively on worf, Apple M5 Pro. These selected target suites use
RUST_TEST_THREADS=1; explicit in-test concurrency remains exercised. This is not
native ARM performance evidence or a release-package qualification.

The initial Mac parallel suite encountered independent fsync load beyond the fixed
250ms retention deadline. ARM parallel suites similarly emitted honest contention
warnings under emulation. Serial isolation avoids unrelated test-process pressure;
the production deadline and warning behavior were preserved. B6 must qualify
realistic product load separately. Earlier failed attempts remain in private logs.

ARM also exposed repeated receipt reconciliation per record. The implementation
now reconciles once per distinct resolved UUID per bounded transaction, with a
regression preserving already-supported identity when new fields are unknown.
The outer contention test is a3second retry/backoff guard that includes process
startup and emulation; direct cumulative250ms foundation assertions are unchanged.
The inherited ten-MiB bulk D120 test has a120second parent harness budget for QEMU;
its exact chunks/body assertions and production60second request deadline remain.

## Boundaries exercised

Named references, revision rotation, endpoint evidence in both arrival orders and
later conflict invalidation; actual INT130/TERM143 with durable progress and resume;
no resubmission or repeated-resume duplicates; stable item IDs across SDK chunks;
failed essential checkpoint preventing history creation; payload/key/snippet absence
throughout archive bytes; payload-registry overflow refusing archive creation;
private path/opt-out and flag-shaped-value parsing; bounded capture and one quiet-
safe warning; complete/pending/failed metadata, independent malicious and allow/block
classification; finalization contention retaining data with honest run status.
D348 pins the frozen TypeScript difference. No additional HTTP request is introduced.

## Raw receipts

- `.local/dev/runtime-history/b2/acceptance3/check-results.json`: SHA256 `777aa5d1f3e9ea5451e2a8021a17ebf05aa3ae919ccd3d9075a5c8391571e06c`.
- `.local/dev/runtime-history/b2/acceptance3/test-summary.json`: SHA256 `ebdd64f5d21ea539b8b96f0564ca0007728c1d178d53626de800cf1796adc0a0`.
- `.local/dev/runtime-history/b2/acceptance3/linux-receipt.json`: SHA256 `bb46cab0829b44a266f8709fd4306d87244ecabc094824be361f947ab4c33cb2`.
- `.local/dev/runtime-history/b2/acceptance3/macos-receipt.json`: SHA256 `7f19a4aa9c83b23956468d681dc5b19634969e920c4e7b5ec600aaf5ce5a4c2a`.
