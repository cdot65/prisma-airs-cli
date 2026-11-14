# CLI9.1.0 release integration: Claude review1

Actual Claude Code review of the registry-pin/version integration. This interim
review scored8/10 while native checks were still running; its prerequisites are
being addressed. It does not replace the earlier gateway implementation review
or claim independent test execution.

```json
{
  "score": 8,
  "assessment": "I found no correctness or security defect in the release-integration code. The pin is consistent and exact. All four SDK consumers (root, airs-runtime-core, airs-history-store, airs-mcp) use `scm = { package = \"scm-sdk\", version = \"=0.2.6\", ... }` with an identical feature list. Cargo.lock has one scm-sdk entry: 0.2.6, the crates.io registry source, checksum 382715ffd429bf4c4b02364131c6bd183775d29e127cc2f3049c56043ee203e6. That checksum is identical to the checksum in the verified registry.json receipt, which also records published=true, downloaded_bytes_identical=true and source commit 2adbac87. The five versioned workspace packages (airs-cli, airs-runtime-core, airs-history-store, airs-mcp, airs-mcp-client) are 9.1.0 in both their manifests and the lock. Every inter-crate dependency uses `=9.1.0`, including the dev-dependency on airs-runtime-core. airs-test-harness stays at 0.0.0, which is correct for an unpublished helper crate. `SDK_VERSION` is \"0.2.6\" and both user-agent assertions expect `PAN-AIRS/0.2.6-rust-sdk`. The runtime-core crate's SDK pin is also 0.2.6, so the version stamp, the pin and the tests agree. The 9.1.0 release notes, DESIGN, COMPATIBILITY and progress text consistently say that the SDK diagnostics are not yet presented through the CLI error bridge, that stderr keeps the existing schema messages, and that live contracts and distribution stay out of scope. The prior 9/10 review's scope exclusions are preserved and not relabelled as accepted. Some remaining notes are stale, and the CLI qualification evidence is only partial. That is why this is 8 and not 9. I did not run anything. The qualification progress file currently lists only three completed entries (fetch --locked, airs-cli --lib with 694 passed, and airs-runtime-core --lib with 240 passed, all on Rust 1.99.0).",
  "findings": [
    {
      "severity": "low",
      "path": "docs/gateway-reliability/IMPLEMENTATION_PROGRESS.md",
      "issue": "The current-status table (line 14) still cites '217 SDK library tests' and, in the G0.2 row (line 13), '79 passing gateway-write tests'. The verified 0.2.6 receipt reports 231 all-feature + 150 minimal + 13 doctests + 1 external consumer, and the prior final review recorded 80 gateway-write tests. Lines 21-30 describe the candidate as pinned to 0.2.6, so the table mixes pre-publication numbers with post-release claims.",
      "required_before_merge": false
    },
    {
      "severity": "low",
      "path": "tests/gateway_inventory.rs",
      "issue": "The comment at line 2461 still says 'The pinned SDK's user agent (scm-sdk 0.2.3, ...)', but the assertion on line 2467 now expects 0.2.6. The comment is stale and misleading about which pin the test follows. The assertion hard-codes the literal and is not derived from `airs_runtime_core::SDK_VERSION`. A future pin bump therefore requires editing three places by hand: the manifests, the constant and both tests. This is acceptable, but a drift check would be cheap.",
      "required_before_merge": false
    },
    {
      "severity": "low",
      "path": "Cargo.toml",
      "issue": "The dependency comment (lines 60-62) credits 0.2.6 with 'published value-free diagnostics' and carries forward the 0.2.5 and 0.2.4 history, but it does not say that the diagnostic getter is unadopted. The release notes, DESIGN and COMPATIBILITY say that explicitly. This is a wording gap only.",
      "required_before_merge": false
    },
    {
      "severity": "low",
      "path": "docs/RELEASE-NOTES/9.1.0.md",
      "issue": "The notes cover behaviour and the SDK dependency but do not say that all five versioned workspace crates move to 9.1.0 in lockstep, or that this is a source release only. The final paragraph says the MCP service is not deployed by the source release. A one-line statement of package and version semantics would remove ambiguity. The README calls this a 'release candidate' while the notes read as final.",
      "required_before_merge": false
    }
  ],
  "required_changes_for_9": [
    "Complete the CLI native qualification run against the registry pin and record the results. At minimum this must cover cargo test --locked --offline on Rust 1.89 and 1.99 for the root package, runtime-core, history-store, mcp and mcp-client, plus the integration tests tests/debug.rs, tests/gateway_inventory.rs and tests/gateway_writes. The progress file currently shows only three entries.",
    "Record Clippy results (with warnings denied) on the 9.1.0 candidate. The 'Clippy clean' statement for the CLI currently rests on earlier snapshots.",
    "Update the stale counts in IMPLEMENTATION_PROGRESS.md (217 SDK tests, 79 gateway-write tests), or label them clearly as historical.",
    "Fix the stale 'scm-sdk 0.2.3' comment at tests/gateway_inventory.rs:2461.",
    "Add one sentence to docs/RELEASE-NOTES/9.1.0.md saying that the five versioned crates move in lockstep to 9.1.0 and that this is a source release candidate."
  ],
  "evidence_read": [
    "Cargo.toml",
    "Cargo.lock (scm-sdk entry and all airs-* package entries)",
    "crates/airs-runtime-core/Cargo.toml",
    "crates/airs-history-store/Cargo.toml",
    "crates/airs-mcp/Cargo.toml",
    "crates/airs-mcp-client/Cargo.toml",
    "crates/airs-test-harness/Cargo.toml",
    "crates/airs-runtime-core/src/lib.rs (SDK_VERSION)",
    "tests/debug.rs (user-agent assertion)",
    "tests/gateway_inventory.rs (lines 2455-2470)",
    "docs/RELEASE-NOTES/9.1.0.md",
    "docs/gateway-reliability/CLAUDE_REVIEW_FINAL.md",
    "docs/gateway-reliability/IMPLEMENTATION_PROGRESS.md (lines 1-60)",
    "docs/DESIGN.md (gateway reliability source amendment)",
    "docs/COMPATIBILITY.md (gateway reliability source additions)",
    "README.md (version lines, via grep)",
    "/Users/cdot/development/goshawks.io/scm-sdk-rs/docs/releases/0.2.6/registry.json",
    "RELEASE_ROOT/cli-qualification-progress.json (partial)",
    "grep sweeps for stale 9.0.x, 8.x and 0.2.3-0.2.5 version strings across src, tests and crates"
  ],
  "verification_prerequisites": [
    "I executed no commands or tests. The 694 lib tests and 240 runtime-core tests in cli-qualification-progress.json are supplied evidence, not mine.",
    "The CLI native integration run is still partial. The remaining items are the MSRV 1.89 runs, the integration test targets (debug, gateway_inventory, gateway_writes), the other workspace crates, Clippy and the generated-reference check.",
    "I did not open the logs under RELEASE_ROOT/logs or verify their SHA-256 values against the progress file.",
    "The SDK package checks (231 + 150 + 13 + 1 on Rust 1.89 and 1.99, Clippy clean) come from the supplied description. registry.json confirms identity, checksum and publication, but not those test counts.",
    "I did not inspect the SDK 0.2.6 source or the .crate contents. 'No behavioural change from the CLI's point of view' is therefore inferred from the unchanged runtime gateway source and the matching pin, not checked against the SDK diff.",
    "Docusaurus typecheck and production build passing (Node 24, pnpm 10.6.5) are as supplied. I did not read build output.",
    "Live service contracts, distribution channels, other platforms and fleet acceptance are explicitly out of scope and are not scored."
  ]
}
```
