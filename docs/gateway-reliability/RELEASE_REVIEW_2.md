# CLI9.1.0 release integration: Claude review2

Actual independent Claude Code review after registry integration checks. Score8/10;
no correctness/security defect found. Remaining merge prerequisites include the
version-sensitive root checks and either Linux MCP execution or explicit owner
acceptance of a CLI-only source merge. The fixed verifier checked source/log hashes;
it did not independently execute tests.

```json
{
  "score": 8,
  "assessment": "The integration code is sound for what it claims to be: a CLI source merge. I found no correctness or security defect in it. Registry pins and version coupling check out against the manifests. All four SDK consumers pin scm-sdk to exactly =0.2.6 with identical feature lists. The five versioned crates are all 9.1.0, their inter-crate dependencies are =9.1.0, and airs-test-harness stays at 0.0.0. The earlier stale-count, stale-comment and wording findings are fixed. The remaining progress-table counts are labelled 'Initial review', and the gateway-write count of 80 matches the prior final review. The qualification receipt does not turn failures into passes. Top-level passed is false and all_workspace_tests_passed is false, next to cli_affected_checks_passed true. The 56 MCP failures are disclosed on both compilers, and the receipt records them in the cli-workspace-libs-1-99-0 and cli-extra-airs-mcp-1-89-0 entries. Why this is not a 9: (1) the full workspace matrix is red, so the merge is not unconditionally green; (2) the nine root integration targets that ran are a gateway-focused selection, not the full root suite, and the 9.1.0 version bump touches areas that other root targets cover (see the finding on version-sensitive root tests); (3) the MCP service, which carries the SDK pin in the airs-mcp crate, has no passing test result on any platform in this evidence; (4) the Docusaurus result has no raw log. I did not execute tests. The verifier confirms hashes and receipt consistency only. I did not open mcp-macos-platform-refusal.json or the logs directly. The verifier is my only check on the 56-failure inventory and the log contents.",
  "findings": [
    {
      "severity": "medium",
      "path": "docs/gateway-reliability/release-9.1.0/qualification.json (checks[cli-integration-*]); tests/release_packaging.rs, tests/npm_package.rs, tests/homebrew_formula.rs, tests/docs_consistency.rs, tests/program_name.rs, tests/ci_scripts.rs",
      "issue": "Only nine root integration targets were executed (gateway_writes/reads/inventory/report/inference, runtime_writes, management, debug, cli_reference). The source manifest hashes roughly 50 root test files. Several of the unexecuted ones (release_packaging, npm_package, homebrew_formula, docs_consistency, program_name, ci_scripts) can be sensitive to a version bump, distribution identity or documentation text, and 9.1.0 changes all of those. The integration document and receipt call the selection 'affected', but no analysis ties that selection to the version-bump surface. A selection is not a regression pass over the root suite.",
      "required_before_merge": true
    },
    {
      "severity": "medium",
      "path": "docs/gateway-reliability/release-9.1.0/qualification.json (cli-workspace-libs-1-99-0, cli-extra-airs-mcp-1-89-0); crates/airs-mcp/src/storage.rs",
      "issue": "56 of 170 MCP-service library tests fail with UnknownFilesystem on each compiler. The storage guard is byte-identical to base e235a4d0 (verifier confirmed the hash and that the nonzero results are exactly these refusals). I see no evidence of a regression. But airs-mcp also carries the =9.1.0 and =0.2.6 pins, and the 56 failures stop before SDK/client construction. The SDK-pin interaction in the service is therefore untested anywhere. The merge is defensible only as a CLI source merge that explicitly does not accept the service. The documents say that. The merge prerequisite is nonetheless open: either Linux execution, or a recorded owner decision that merging with a red service suite is accepted.",
      "required_before_merge": true
    },
    {
      "severity": "low",
      "path": "docs/gateway-reliability/release-9.1.0/qualification.json (docs_site)",
      "issue": "The Docusaurus typecheck and build results were observed in tool output and no raw log was retained. The receipt says so honestly, but this is the only check in the matrix with no hash-bound artifact.",
      "required_before_merge": false
    },
    {
      "severity": "low",
      "path": "docs/gateway-reliability/release-9.1.0/qualification.json (cli-workspace-clippy, cli-clippy-release)",
      "issue": "Both Clippy runs, including workspace/all-targets, were executed on Rust 1.99.0 only. Tests ran on both 1.89 and 1.99. The 1.89 lint level is not covered, so the Clippy claim should stay scoped to 1.99.",
      "required_before_merge": false
    },
    {
      "severity": "low",
      "path": "docs/gateway-reliability/IMPLEMENTATION_PROGRESS.md:13-23",
      "issue": "The status table still mixes pre-publication numbers with post-release claims. It cites 217 SDK tests under 'Initial review' next to the 231/150/13/1 counts for 0.2.6. Each is labelled, so this is not misleading. The table is still easy to misread without the 'Initial review' qualifier.",
      "required_before_merge": false
    }
  ],
  "required_changes_for_9": [
    "Run the remaining version-sensitive root integration targets (at minimum release_packaging, npm_package, homebrew_formula, docs_consistency, program_name, ci_scripts) on Rust 1.89 and 1.99, with locked/offline and hash-bound logs. Alternatively run the full root test suite, or add a short written analysis showing that the nine-target selection covers everything the 9.1.0 change affects.",
    "Resolve the MCP service gate. Either run the airs-mcp library suite on an admitted Linux filesystem and retain a receipt in which the 56 storage-dependent cases pass, or record an explicit owner acceptance that this merge proceeds with the MCP suite unexecuted. Do not change or bypass the storage guard.",
    "Retain a raw log, with a hash recorded in qualification.json, for the Docusaurus typecheck and build. Otherwise keep the claim worded as 'observed' and exclude it from the passing summary.",
    "Either run workspace Clippy on 1.89 or keep the Clippy claim explicitly scoped to 1.99 in the integration document and receipt."
  ],
  "evidence_read": [
    "docs/RELEASE-NOTES/9.1.0.md",
    "docs/gateway-reliability/INTEGRATION_9_1_0.md",
    "docs/gateway-reliability/IMPLEMENTATION_PROGRESS.md (lines 1-80)",
    "docs/gateway-reliability/RELEASE_REVIEW_1.md",
    "docs/gateway-reliability/release-9.1.0/qualification.json (full: manifest, 17 check entries, scope, comment_only_followup, candidate_binary, docs_site, limitations)",
    "Cargo.toml, crates/airs-runtime-core/Cargo.toml, crates/airs-history-store/Cargo.toml, crates/airs-mcp/Cargo.toml, crates/airs-mcp-client/Cargo.toml, crates/airs-test-harness/Cargo.toml (version and scm-sdk pin lines, via grep)",
    "Fixed verifier output (JSON)"
  ],
  "independent_verification": {
    "ran": "verify-cli-release (fixed read-only verifier, executed exactly as instructed)",
    "result": {
      "hashes_verified": true,
      "cli_affected_checks_passed": true,
      "workspace_all_targets_clippy_passed": true,
      "mcp_full_suite_passed": false,
      "mcp_mac_storage_refusals_per_compiler": 56,
      "source_files_verified": 371,
      "logs_verified": 15,
      "registry_pin_verified": "0.2.6",
      "candidate_version": "9.1.0"
    },
    "interpretation": "This is independent hash and receipt verification only, not independent test execution. It confirms that the receipt matches the current source and logs, that the pins are consistent, and that every nonzero result is one of the 56 MCP filesystem refusals. It does not show that the tests ran as logged beyond what the logs contain. I did not read the logs or the mcp-macos-platform-refusal.json inventory directly, so their contents are known to me only through the verifier's checks."
  },
  "remaining_limitations": [
    "The full workspace test matrix is not green: MCP-service 114 passed / 56 failed on each of Rust 1.89 and 1.99 (macOS ARM64), all UnknownFilesystem at the unchanged storage guard.",
    "Linux service qualification is absent, and MCP service deployment is excluded from this merge and has not been accepted.",
    "Execution covers macOS ARM64 only, on the synthetic/loopback fixtures in the receipt. Other platforms, the fleet and live services are unqualified.",
    "The M7 older-binary integration is outside the executed selection.",
    "The original live service contracts are unresolved: the organisation-info response mismatch, the service-key type vocabulary and deployment replace/merge semantics.",
    "CLI field-diagnostic presentation through the error bridge is not adopted. Stderr keeps the existing schema messages.",
    "CLI distribution artifacts are not published by this merge.",
    "One root-library test is ignored because it is a subprocess lock helper. Four history-store scale/memory cases are ignored as opt-in qualifications and were not run, and no scale acceptance is claimed.",
    "The Docusaurus result has no raw log, and Clippy ran on 1.99 only."
  ]
}
```
