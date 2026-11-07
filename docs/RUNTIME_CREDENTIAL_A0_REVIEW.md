# Runtime credential contract — independent A0 review

Reviewer: independent Codex instance `/root/phase4_review`, 2026-11-04.
Scope: A0 only, against the approved runtime-history plan. Named scanner command
integration, schema changes, routing, snapshots and bulk-state version 2 remain
future A1 deliverables. This review does not claim those commands exist.

## Initial assessment

Score: **8/10 — revise before A1**.

Reviewed contract SHA-256:
`26d37c6e95aa18c0c50f4914123b67a6752ecf3bf80005b84011df5eb9a06547`.

One blocking contract mismatch: `bind-default` promises to copy the effective flat
retry setting but specifies a default of 0. Current `build_scanner_bounded` passes
`TenantConfig::num_retries()` through unchanged; an absent `airsNumRetries` becomes
`InitOptions.num_retries = None`. Pinned scm-sdk 0.2.4 `configuration.rs:90` uses
`unwrap_or(5)`, and its initial configuration also uses 5. New `set` input may
intentionally default to 0, but copying the flat default must materialize 5 to
honor the effective-setting promise, or explicitly declare a policy change.
Bounded lookup/report reads should still override retries to zero as specified.

A small wording issue identified earlier has already been corrected: the 2048-byte
named-secret limit is the CLI compatibility limit, within the SDK's 16-KiB header
bound; it is not the SDK's own maximum.

## Findings that hold up

- The identity table matches immutable SDK model types. `ApiKey.customer_app_id`
  is optional, `CustomerAppWithKeys.customer_app_id` is required, and deployment
  metadata has names/auth codes without a typed key-ID link. Dashboard exact
  application ID/name buckets remain a separate namespace. Scanner result
  extensions cannot establish typed application ownership.
- One explicit application binding per named set, with unique exact application
  IDs and case-collision checks on selector names, is implementable. It avoids
  credential probing, display-name joins and unbound flat fallback for discovered
  applications. Operator assertions and unknown upstream semantics are explicit.
- Named sets contain exactly one secret plus their own admitted endpoint/retries.
  Flat operations preserve existing dual-header behavior. `bind-default --use`
  resolves the two-secret ambiguity without claiming server precedence, and the
  copied binding does not silently follow later flat rotation.
- The optional structured TOML namespace is compatible with new-reader/old-file
  behavior and explicitly breaks old-reader/new-file compatibility only on an
  authorized write. Canonical empty markers prevent accidental implicit downgrade.
  Public views expose no secret fragments, auth codes or secret-derived hashes.
- First-upgrade and pre-restore snapshots have explicit private filenames, tenant
  and TSG checks, same-lock ordering, no-replace publication, durability before
  replacement, safe rollback ownership, and local listing/deletion rules. Extending
  Transaction is necessary and feasible; ordinary readers still do not write.
- Legacy bulk state stays version 1; named submissions use a required public
  reference in version 2. The existing old parser checks newer versions before
  decoding strict fields, so it can refuse version 2 as newer. Named resume uses
  the saved selector automatically, rejects removed/rebound/endpoint changes and
  never resubmits accepted work to discover authorization. Same-app rotation is
  explicit and visibly unqualified.
- The new fixed error vocabulary and named-only doctor behavior are coherent.
  Doctor can report configured named credentials while skipping an unselected
  default probe, rather than choosing the first or trying every secret.

## Executable evidence

Independent run: `cargo test --test signal_sdk`, **4 passed**, no failures or
ignored tests; log `/tmp/airs-runtime-a0-independent-sdk.log`.

The three new checks prove optional versus required associations, preserve distinct
IDs despite equal names, show untrusted application extensions being dropped by the
actual direct-results CLI, and inspect both legacy scanner headers on loopback.
The existing chain test remains a synthetic SDK compatibility check. These tests
establish local shapes/wire behavior, not production ownership or post-rotation
lookup authorization. COMPATIBILITY correctly labels command integration as A1.

Required fmt and Clippy records currently show exit 0; the full mandatory-parity
workspace run remains in progress. Its final result must be recorded before A0 is
accepted. No private bundles, real tenant configuration/credentials, live APIs or
SDK modifications were used in this review. Only this review file was written.

## Reviewed hashes

SHA-256 values for the reviewed inputs (SDK paths relative to the installed
immutable `scm-sdk-0.2.4` crate):

| Input | SHA-256 |
| --- | --- |
| `tests/signal_sdk.rs` | `013e69747815f103e03eaa2146982bca7f30b2c9cef5f92f23cf59932ce9ca19` |
| `docs/COMPATIBILITY.md` | `88edc517ca4c29bcd738280c39fad17acc93470ea58a19ee8f0c35036202583e` |
| `docs/RUNTIME_HISTORY_IMPLEMENTATION_PLAN.md` | `9ee5be3c1147ad07f3831acb1558b6eca09fdbd010f7720a321c0f11a0452d77` |
| `docs/RUNTIME_HISTORY_CLAUDE_REVIEW.md` | `15cba640a55d69e86f42d88c7f3b382fa19c7931395ff798cc36bc618204e317` |
| `AGENTS.md` | `686576e2063cb292eccf7e4ce458afe06964e8ec6577ddc35151cdf29f0f4661` |
| `docs/DESIGN.md` | `8f6c343eba4e50e709d37ad2894c83bb543a3fccd2630054d77b21be62a507e2` |
| `src/tenant/config.rs` | `dca2623b12e3e36d6bcdedfd056ad0a95aeff552a5082c5d131c9166d3ef20f0` |
| `src/store/atomic.rs` | `ad7a5891b89124df26cc8c81025d59cdb9497d08ad1b26c40212b7fe8d1a7ed6` |
| `src/clients.rs` | `83385402265ebce7c2a726f2476e53e641617203354526a8cfa70ecabc0e72ae` |
| `src/bulk/state.rs` | `039a5df00b81310375c6e8bfc59456815d8f94eccebc9852dfe066543c1f878b` |
| `src/bulk/engine.rs` | `457d7ec2303e64f1c535f31a5852d622f21ac4722a8b59774d000ed22386ce2a` |
| SDK `src/models/generated.rs` | `9e8de4e2a9d890da9c54a769c48f7f68117fc560c2ad8fc42932e195d7c540a3` |
| SDK `src/auth.rs` | `e8bfe692101737c13b278b60b8b1bf22b861ab18cd6eec42276fedd5b8a255f1` |
| SDK `src/airs/scan/scanner.rs` | `f34c74ba224da093f71052a529ffbcd7505d3ee073d13cd1cee9f5af17a15159` |
| SDK `src/configuration.rs` | `a0dcae9ab174e2b6561d01d750574cd2c3c6fb7be161ef681b6ca6f6f7b19683` |

## Re-review: effective retry default corrected

The contract now copies configured `airsNumRetries`, otherwise the pinned SDK's
actual default of 5. New `set` input still intentionally defaults to 0. The amended
SDK dual-header test directly asserts `configuration.num_retries() == 5` when no
retry setting is provided. Independent focused rerun: **4 passed**; log
`/tmp/airs-runtime-a0-independent-sdk-final.log`.

Updated reviewed hashes:

| Input | SHA-256 |
| --- | --- |
| `docs/RUNTIME_CREDENTIAL_CONTRACT.md` | `50181bd5e9f263639ba45c8fd5b58c88b130a1d98ece12aa18f3c1aecd44876d` |
| `tests/signal_sdk.rs` | `05ce9ba551c0a0a4f537caf0d4ac4646d43c8597850fff0c02d1833dbd714014` |

The initial full workspace run exited 101 at the unchanged paired test
`parity_redteam_reads::divergence::d142_downloads_are_private_and_never_overwrite`:
the frozen TypeScript download was mode 0600, while the test expects 0644. This is
consistent with a restrictive inherited umask, not an A0 production-code change.
A rerun with the test environment's expected umask is required; the failure is
recorded here and is not represented as a passing check. A0 acceptance remains
pending that result; no remaining A0 contract blocker was found after correction.

## Final assessment — A0 accepted

Final score: **9/10 — A0 accepted; A1 may begin**.

Final reviewed contract SHA-256:
`9215881676b2e55d36dac58841230f36e1fe006d6e97e958deeac12e29cb60de`.
Final reviewed SDK-test SHA-256:
`05ce9ba551c0a0a4f537caf0d4ac4646d43c8597850fff0c02d1833dbd714014`.
COMPATIBILITY remains at the reviewed hash above.

The retry-default blocker is resolved and executable-tested. The final endpoint
clarification uses the existing admitted SDK URL canonicalization and trailing-slash
removal for storing/comparing named endpoints, avoiding false resume mismatches
without changing legacy state interpretation. The strict grammar, fixed diagnostics,
masked metadata, binding policy, downgrade recovery and versioned polling reference
now form a concrete A1 implementation contract. No remaining A0 blocker was found.

Independently inspected `.local/dev/runtime-history/a0-recheck-results.json` and
recomputed the summary directly from `a0-workspace-recheck.log`:

- `cargo fmt --all --check`: exit 0.
- `cargo clippy --workspace --all-targets -- -D warnings`: exit 0.
- Mandatory-parity workspace tests: exit 0; **3697 passed, 0 failed, 5 ignored**,
  across 90 result groups; **0 `PARITY SKIPPED` notices**.
- Independent final SDK-focused run: **4 passed**, including default-retry and
  dual-header assertions.

The recheck ran in a child with umask 022, matching the frozen TypeScript parity
mode expectation; private validation logs remain private. The original exit-101
run and initial 8/10 assessment above are retained. No test privacy assertions or
production code were weakened to obtain the passing recheck.

This acceptance covers A0's contract and synthetic capability evidence only. A1
must implement and test the proposed commands, transaction ownership/durability,
application-scoped wire isolation, migration/downgrade behavior and bulk-state
compatibility, with DESIGN and paired difference records as behavior lands.
Real-service app ownership and post-rotation authorization remain user-run
qualification questions, explicitly acknowledged rather than inferred from tests.
