# B3 synchronization qualification

Status: full default GNU baseline and all three platform suites passed.
Independent Codex `/root/b3_review` accepted B3 at **9.2/10**.
The current gate is an independent assessment of at least 9.1/10 (AGENTS.md).
B4 portability/pruning may proceed under its own acceptance gate.

## Frozen executable inputs

Version: 8.2.0-dev.1. Acceptance5 contains 492 public source/documentation files.
The manifest SHA256 is
`645e910de5e757d66e8562c9512275eadeddd684d6cb735fe4b3a4893d97c4b7`.
The manifest and source archive are retained under
`.local/dev/runtime-history/b3/acceptance5/`. Independent review verified every
registered file and archive member, with no unsafe paths or links. No private
validation bundles, tenant credentials, SDK checkout, or logs are in that snapshot.
Qualification/review documents added afterwards describe that frozen implementation.

The SDK remains exactly 0.2.4. SQLite is bundled. These are debug functional
qualification artifacts, not signed release packages or native ARM performance
measurements. The existing 8.1.0-rc.4 test packages and target/release binary are
preserved. No commit, tag, release, signing, publishing, live API test or unattended
scheduler installation was performed.

## Required baseline checks

| Check | Result |
| --- | --- |
| cargo fmt --all --check | Pass |
| Workspace/all-target Clippy, warnings denied | Pass |
| AIRS_REQUIRE_PARITY=1 cargo test --workspace --locked | 3,833 passed, zero failed, five existing ignored tests, 98 result groups; zero parity skips |
| Rust 1.89 workspace/all-target check, locked | Pass |
| Documentation production build | Pass |

The workspace run uses default test scheduling; the orchestration explicitly removes
RUST_TEST_THREADS. Node 24 executes the pinned unmodified TypeScript reference.
Private receipt orchestration uses umask 077, while child checks use 022 to preserve
the reference CLI's file-mode contract. Required checks took 314.52 seconds for the
workspace, 0.28 for the MSRV check, and 1.28 for the documentation build.

## Selected platform suites

Each complete platform run includes 48 focused history units, 84 scan tests,
64 bulk tests, five inspection tests, ten import tests, seven automatic-retention
tests and 21 sync CLI tests: 239 passes plus one existing ignored bulk helper.
All platform compilers are Rust 1.89.0. Platform test-case scheduling is serialized;
actual threads, reader/writer contention, signals and child processes within each
case remain concurrent.

| Target | Environment | Result |
| --- | --- | --- |
| x86_64-unknown-linux-musl | Native enterprise | 239 passed, zero failed, one existing ignored helper |
| aarch64-apple-darwin | Native worf, Apple Silicon, Darwin 27.2 | 239 passed, zero failed, one existing ignored helper |
| aarch64-unknown-linux-musl | QEMU user emulation on enterprise; child CLI via binfmt | 239 passed, zero failed, one existing ignored helper |

The Mac used an isolated temporary copy verified against the source manifest, with
an external Cargo cache. Its source copy was removed after qualification. The
two musl targets are statically linked. Final binaries and receipt hashes are
recorded below. All registered executable inputs matched the manifest at completion.

## Findings, corrections and execution limits

Early review identified incorrect current-reference restamping, insufficient
attribution context, late range/privacy filtering, sticky HTTP status, native scope
labels, normalization and validation ordering. Later review found unsupported report
candidates hiding eligible work, cancellation before transport consuming an ordinal,
optional telemetry masking failed primary reads, and missing clock diagnostics.
Focused regressions cover the corrections, including three scanner applications
sharing the same global two-report-send budget and cached examples avoiding refetch.

Privacy projection must distinguish a supplied constraint that was removed from a
field genuinely absent at source. A non-sensitive boolean now blocks receipt
promotion, invalidates a prior promotion on later flagged evidence and rebuilds the
affected projection. Native/legacy normalization, received report metadata and a real
two-sync CLI sequence against an existing full bucket test that distinction. The
unpublished schema requires an explicit receipt-binding boolean; older development
signatures are refused unchanged. No migration from draft archives is claimed.

The default parallel library pilot and separate audit showed unrelated synthetic
archive fixtures could saturate the shared test volume and exceed the unchanged
production 250ms retention deadline. A test-only process-local gate now surrounds
independent history unit fixture creation and cleanup. No timer, warning assertion,
production deadline, path/endpoint guard or intentional within-case concurrency was
relaxed. Child helper processes have separate test gates. The default workspace
baseline passes with this isolation.

An initial ARM64 bulk run overlapped the full default workspace run. Three exact
stderr assertions observed real retention-deadline warnings: 61 passed, three
failed, one existing ignored helper. That run remains a failed, preserved result;
it is not counted as successful qualification. An isolated rerun subsequently exposed an unrelated timeout-test assumption: all
primary reads may be incomplete inside a one-second configuration stage, correctly
producing unavailable JSON and exit1. The test now requires explicit availability
to determine the exit code, while preserving all admitted-read journal assertions.
Three legacy bulk byte-format/state cases explicitly opt out of history per
invocation; their assertions remain unchanged, and dedicated retention tests retain
the default path. No shared helper injects an opt-out.

The final frozen acceptance5 ARM64 suite then ran alone after all local baseline
and native checks finished: every selected group passed. Production sources are
byte-identical to acceptance4; the final delta is tests/documentation only. This
isolates emulation/host load without changing production limits. The isolated pass
establishes functional behavior under those emulated conditions, not native ARM
performance or robustness under simultaneous host saturation; B6/system qualification
retains those limits.

Acceptance1–4 and pilot outputs remain preserved with their superseded scope.
They include a discovered privacy blocker, an expanded synthetic report fixture with
an inconsistent sampling list, and an inspection seed lacking the new explicit
privacy marker. Fixture corrections preserve strict validation. Only the final
frozen proof and its disclosed execution conditions can support B3 acceptance.

## Receipt locations

Required-check summary and complete log:
`.local/dev/runtime-history/b3/acceptance5/check-results.json`,
`test-summary.json`, and `workspace.log`.
Native Linux: `linux-platform-results.json` and target-specific logs.
Earlier failed emulated attempts are preserved under the sibling acceptance4
directory and its arm-isolated subdirectory.
Native Mac: `macos-receipt.json`, `macos-foundation.log`, and
`macos-inspection-import.log`.
Isolated ARM64 rerun: `arm-isolated/` under the same acceptance directory.
The successful isolated ARM run is recorded separately from earlier failed attempts.

## Frozen debug CLI artifacts

These private retained executables identify the tested development build; they do
not replace the existing release binary or constitute signed test packages.

| Target | SHA256 |
| --- | --- |
| x86_64-unknown-linux-musl | `8b1ff377085e14beee5fab3804f990582e021b3e062195a6b245ef75aba30697` |
| aarch64-unknown-linux-musl | `dc0dfec0d03dfd1a0b48d3b298ba9bfc3b40164d8fe217058076f00fe830e4d7` |
| aarch64-apple-darwin | `23db0553e82fc36afc8b98a4bd726e6add85e554607930bec6d85a3823c6ee6c` |

All three are retained under
`.local/dev/runtime-history/b3/acceptance5/accepted-binaries/`.

## Final raw receipt hashes

- `check-results.json`: `982ede268f536b9d3689255f3ea2990cf0618ad4346f3a1772d7b12d0d2ee744`.
- `test-summary.json`: `4ba94c593567c7d4b99b1e1e11b81a77035766fa3e485b4bdb13ac8b8cd94c26`.
- `linux-receipt.json`: `a63dce40c4eb862ac43cfe80f996c46d229cacd84334888d1d9e674a559991f9`.
- `macos-receipt.json`: `4b262867368a96ae0560446636febf51c546c65da2eca2daea01fc2a0081bad2`.
- `macos-binary-receipt.json`: `d58aca29b44234171474d2b93b8272244a9ace3e6de3b037079fe85795a5536a`.

The qualification dossier is updated after freezing executable inputs; no product
source or executable test changed after the final snapshot. Final acceptance and
remaining B4–B6 work are recorded in the phase review ledger.
