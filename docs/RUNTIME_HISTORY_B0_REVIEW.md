# B0 independent review — SQLite contract and isolated feasibility

Reviewer: independent Codex `/root/phase5_review`. B0 is complete with final independent acceptance **9/10**. This accepts the
contract, schema and isolated prototype only; it does not qualify CLI persistence.

## Initial assessment — 8/10

The reviewer accepted the coherent ordinary-read and exclusive maintenance
design resolving Claude C1. Early findings drove these corrections:

- Enforce promoted receipt/event-reference consistency and restrict deletion until
  an explicit transactional demotion; never leave promoted orphans after prune.
- Preserve actual scanner attempts, retry epochs and reset/authorization history
  separately from mutable scheduling state.
- Separate management traversal reads from the scanner-only three-attempt rule.
- Specify versioned logical transfer, bounded decoding, digest/relational validation,
  private validated staging and migration backup manifests.

The provisional 8/10 assessment retained one blocking qualification gap: the first
Mac probe used SQLite's `busy_timeout(250)` and measured 289 ms; its assertion
allowed nearly one second. That did not prove the required cumulative 250 ms
gate-plus-database contention policy. The reviewer requires a captured monotonic
deadline covering both waits, a tight documented scheduler allowance, and completed
Linux musl million-row receipts before final acceptance. The original pilot/final
attempt logs remain under `.local/dev/runtime-history/b0/`.

## Final assessment — 9/10, accepted

The reviewer verified that the four executable-source hashes match both final
acceptance2 platform receipts, all seven Linux checks and native Mac Rust1.89
checks pass, and all three million-row workloads finish within the read budget.
The cumulative contention probe uses one captured Instant across the gate and
SQLite, no fresh timeout, and at most1ms/remaining callback pauses. Every target
measures250ms total. The reviewer independently reran the isolated offline
Rust1.89 tests: **2/2 passed**. No B0 blockers remain.

The qualification accurately discloses emulation, warm OS cache, allocator versus
RSS limits, the observed initial277.7ms write tail, and deferred B1–B6 product/
durability gates. Root Cargo.toml/Cargo.lock contain no SQLite at this acceptance
point. B1 may adopt the qualified binding; automatic persistence waits for B2.

Reviewed fingerprints:

- `docs/RUNTIME_HISTORY_STORAGE_CONTRACT.md`: `2bb132ffb253be547fa7a87be24cc586ec0f0087054929634557fd8507da2d2f`
- `docs/RUNTIME_HISTORY_B0_QUALIFICATION.md`: `8037ae3bd9ff1832e624e323f2d9a88b3fe4c778bf3b3a6a1907f9dc5b372bd0`
- `prototypes/runtime-history/Cargo.toml`: `94876efa5b38defad89c73a55b69edb4ee23c17ebe8dbcba683bd71662624931`
- `prototypes/runtime-history/Cargo.lock`: `fefcf60850bc2361bda53697694db90df71c5d897a5b7b5e864bf18d1463da6d`
- `prototypes/runtime-history/src/main.rs`: `ddef651ae04f085912d4798ebcbb3e9c919d0ef28ee35e2cf900a70373a5a574`
- `src/history/schema.sql`: `a8f92645e56f14b6b129ae86141bd0ea4c4a7a7d2c4f0c0b36885e31d130df01`

Final receipts: `.local/dev/runtime-history/b0/acceptance2/linux-receipt.json`
SHA256`f63c543fe762d8e2793b3fafc57a3b7b642ddb379c8123cd7593638267533a25`;
`macos-receipt.json` SHA256
`6558d7786e633b7a20d74f14f7d4b1c10b85be1add73936db9a39bd12e196dec`.
