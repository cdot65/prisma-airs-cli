# Runtime history A3 — RC4 qualification review

Reviewer: independent Codex instance `/root/phase4_review`, 2026-11-04.
Scope: 8.1.0-rc.4 qualification only. A0/A1/A2 have their own independent acceptance
records. This review does not qualify real-service ownership or rotation access.

## Initial qualification review

Initial readiness score: **8/10 — awaiting platform artifact evidence**.

The required checks are complete and strong. Independently parsed the workspace
log: **3735 passed, 0 failed, 5 ignored**, 93 result groups and **0 parity skips**.
The recorded fmt, Clippy, full mandatory-parity workspace, Rust 1.89 all-target and
documentation-build checks all exited 0.

Source provenance was checked directly: `SOURCE-SHA256.json` has 406 entries;
`source.tar.gz` has exactly those plus the manifest, all regular files, no duplicate
names, and every extracted byte sequence matches its recorded SHA-256. No absolute
or parent-traversal path, .git/.local content, SDK checkout or private customer
validation directory is present. The manifest SHA-256 is
`3f890dacef9b761a4ad45cf4bc5e3ac54da754fe24f087830e27acc2c861047e`.
The listed workspace source files match that snapshot.

The reviewer independently ran the actual host-native GNU release binary through
both legacy-v1/current-v2 offline bundle/review/no-overwrite smokes and the RC4
credential loopback smoke. All passed. The credential smoke made seven loopback
requests (one Reports send) and verified masked readers, exact private snapshots,
key/token request isolation, named bulk version 2, removed binding refusal without
a send, rotation resume without resubmission, and exact-byte restoration. This
reviewer's invocation did not include the frozen old binary; the separate final
qualification receipt must establish old-binary refusal.

Reviewed automation uses isolated synthetic stores and loopback endpoints. The
Mac source transfer validates regular safe archive paths and each source hash,
builds offline, runs native release tests and actual native/extracted artifact
smokes, checks an ad hoc signature, and records the source fingerprint. Linux
cross-compilation alone is insufficient: the arm64 ELF and its extracted aliases
must execute under the documented emulator before final acceptance.

Initial guide findings: references to RC2/RC1 and a one-call default were stale
under per-app routing. They have been corrected to RC4, a metadata-only baseline,
and the global two-send allowance across credential groups. The guide correctly
separates operator binding assertions from proven ownership and reserves actual
service qualification for the user. A final clarity request is to stop the remaining
optional Reports qualification after any HTTP429 or authorization denial, including
skipping the later enriched-report step.

At this initial review point, all three final platform packages and their complete
native/emulated/extracted receipts were not yet available. Final score >=9 is held
until that evidence, package hashes/layout, source consistency and the shipped guide
are verified. No implementation or publishing workflow changes were made by the
reviewer; only this review file was written.

## Intermediate artifact checks

All three combined archives are now present. Independent verification confirmed
SHA256SUMS, exactly six expected entries, root ownership, executable/text modes,
relative compatibility aliases and the binary hash against BUILD-INFO for each
archive. The shipped TESTING.md and RELEASE-NOTES.md match the current repository
documents. The guide now explicitly skips the remaining Reports qualification
after HTTP429 or authorization denial.

The reviewer independently executed the actual static aarch64-musl ELF through
the registered QEMU binfmt handler, including offline-v2 replay and the loopback
credential smoke. Then the final aarch64 archive was extracted into a fresh
private temporary directory: all three aliases returned 8.1.0-rc.4, and its actual
extracted ELF passed the credential smoke with the frozen RC3 native binary
included. This independently established both old named-table refusal and old
version-2 bulk-state refusal, with no lookup send or accepted-work resubmission.
The frozen executable is hash-checked by the smoke against
`0856de0c6611239849a8b70f95f03af91940975865849c92334a9c4a3e08ce94`.

The native Mac build receipt matches the source manifest and local Mach-O binary,
and records successful native release integration suites, ad hoc signature
verification, native/extracted offline-v1/v2 and credential workflows. The exact
combined Mac archive still needs its final native receipt. ARM selected release
integration suites are also still running. The final score remains held pending
those two completed receipts and final BUILD-INFO consistency.

## Final qualification review

Final score: **9/10 — A3 accepted for the unpublished RC4 validation build**.

All three platform packages now have completed qualification evidence. Final
BUILD-INFO has `qualification_complete: true`, no pending fields, complete ARM
selected-suite receipts and the final combined Mac package receipt. Its SHA-256
is `534c9dcf4d3869876f8da463e30991ffae2610b1089d9cc3b65f9e34161ea0f6`;
the reviewer independently read the same hash from the copy on worf over SSH.
The final source inputs, archive checksums, shipped guide and release notes were
checked again and remain consistent.

The first ARM selected-suite execution used Cargo's test-build CLI, whose dev
feature resolution produced different bytes from the frozen shipped ELF. That
was a qualification precision gap, not a functional failure. The author reran all
four release integration suites against the actual frozen package binary using
a private test-only rustc wrapper. It overrides `CARGO_BIN_EXE_airs` for exactly
the four integration-test crates and otherwise forwards rustc unchanged. It does
not alter production source or artifact bytes. The reviewer checked each recorded
test executable's SHA-256, confirmed its embedded literal path points to the
frozen packaged ELF, matched the CLI hash, and independently parsed the final
logs: **62 passed, 0 failed, 0 ignored** under QEMU. The corresponding native Mac
selected suites also passed **62/62**.

Final combined Mac qualification executes the exact archive shipped alongside
both Linux archives. The reviewer matched its archive, binary and source hashes,
and verified successful native signature/alias/version, offline-v1/v2, credential
loopback and malformed-alias refusal receipts. The remote qualification verifies
all three archive checksums. This establishes actual execution for every shipped
target; ARM qualification is explicitly emulated rather than claimed as native.

The artifact and config workflows have independent evidence for:

- New/extracted executable versions and help, relative aliases, tar layout/modes,
  private bundles, immutable saved v1/v2 evidence, review replay and no-overwrite.
- Named key/token wire isolation on loopback, exact private upgrade snapshots,
  masked readers, bound bulk state version 2, removed binding refusal without
  network work, rotation resume without resubmitting accepted scans and exact
  downgrade restoration.
- The frozen older native CLI refusing both named tables and newer bulk state,
  including the reviewer's own extracted-ARM/current versus old-native probe.
- Required full checks: **3735 passed, 0 failed, 5 ignored, no parity skips**;
  fmt, Clippy, Rust 1.89 all-target checking and documentation build all pass.

Linux toolchains are now recorded: Rust 1.99.1, cargo-zigbuild 0.23.4, Zig 0.17.0,
QEMU aarch64 11.1.2 and the separate Rust 1.89 MSRV check. Native Mac records Rust
1.99.0, an ad hoc signature and no notarization. Both Linux executables are verified
static ELF files, and the Mac executable is verified arm64 Mach-O. The unchanged
production source snapshot remains at the 406-file manifest fingerprint above.
No signing/publishing workflow change was found in the reviewed diff.

### Final artifact fingerprints

| Target | Binary SHA-256 | Archive SHA-256 |
| --- | --- | --- |
| x86_64-unknown-linux-musl | `3ec550131b7fad8d5bf41291174c81c17d062371f358d3a2ab09465c5b1557bb` | `eed0f3b4a1975f9a87b0e16b4847b78db1024defdd29fad7b3602f5e3b6cc4c3` |
| aarch64-unknown-linux-musl | `6b0685f5010f55a621380eb0135f6a067066fcb2495af628da3a4ae3005c4af2` | `8a172e734810401367d1634010ce0791e36f8a1547fbe407a0809d6a9358db56` |
| aarch64-apple-darwin | `fcb881fcfe9e1d24965047851f75cd8f99daeb5918eddef00426c8c1a3649f32` | `d89199afcb5452cc0ba377335b4c84e810c98a901f607cd8cca2d53cb72a9866` |

Host GNU release binary:
`25781fcdf46849ee6243367881fa655bef8a97c094e979db68a316df664585c1`.
Guide SHA-256:
`bd8860bf28b4951f6bc22892955a34c680178df0b265caab78463fb61040d7cf`.
Release-note SHA-256:
`18243ee2fe17f5965f124a7c4cc7e875e2c13494ea0f6a2672c0be5a6cf7d84d`.

### Limits of this acceptance

This is RC qualification, not stable-release or real-service approval. The guide
correctly treats application bindings as operator assertions, reserves wrong-key
and post-rotation service questions for explicit user-run qualification, and caps
the optional recipe at three Reports sends with a stop after HTTP429/authorization
denial. No live tenant, real credential, private customer bundle, SDK edit, commit,
tag, publication or workflow change was used by the reviewer. Only this review
file was written in the repository; independent probes used synthetic temporary
stores and loopback endpoints. Persistent history remains the separate 8.2 track.
