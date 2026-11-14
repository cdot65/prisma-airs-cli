# CLI9.1.0 final release-integration review

Actual Claude Code re-review3 scored **9/10**, with no required source changes
before the scoped CLI-only merge. The owner accepted deferring Linux MCP
qualification. All failed MCP results remain failed; this is not full-workspace,
service-deployment or distribution acceptance.

The reviewer independently ran the fixed read-only verifier:379 source/input
files and19 logs matched, as did the immutable SDK pin and recorded owner
decision. It did not independently execute tests. Earlier direct source reviews
and the full native evidence retain their original scopes.

Actual model: `claude-sonnet-5-5`. The raw tool envelope is retained in
[the review receipt](release-9.1.0/claude-review-final.json).

```json
{
  "score": 9,
  "assessment": "No correctness, security or release-integrity defect was found for the scoped CLI 9.1.0 source merge pinned to published scm-sdk 0.2.6. All four prerequisites from the 8/10 review are now resolved. The version-sensitive targets, raw Docusaurus logs, explicit Rust 1.99 Clippy scope and the owner's scoped CLI-only merge decision are all recorded. The docs state plainly that the full workspace is not green, with 56 MCP failures per compiler at the Linux filesystem guard. The owner decision stays separate from the test outcomes, and the receipt keeps passed:false and all_workspace_tests_passed:false. The score covers the source merge only. It does not cover MCP deployment, Linux service acceptance or distribution publication.",
  "findings": [
    {
      "severity": "info",
      "path": "docs/gateway-reliability/release-9.1.0/owner-merge-decision.json",
      "issue": "The recorded owner reply ('got it, sure, proceed as you feel is best') is a delegated acceptance. The explicit choice and the filesystem explanation preceded it, and the record limits its scope: source merge only, MCP Linux qualification deferred, and no deployment or publication. That is adequate for a source merge. It is not evidence that MCP is qualified.",
      "required_before_merge": false
    },
    {
      "severity": "info",
      "path": "docs/gateway-reliability/INTEGRATION_9_1_0.md",
      "issue": "The full workspace matrix is not green. MCP-service shows 114 pass and 56 fail per compiler, all UnknownFilesystem. The docs, receipt and verifier output (mcp_full_suite_passed:false) all disclose this consistently. This is an accepted, deferred limitation rather than a hidden defect.",
      "required_before_merge": false
    },
    {
      "severity": "low",
      "path": "docs/gateway-reliability/release-9.1.0/qualification.json",
      "issue": "Optional polish: the workflow fixture and the retained CI-script checks are synthetic. They are not a new runner qualification, and the docs say so. No change is needed for the source merge.",
      "required_before_merge": false
    }
  ],
  "required_changes_for_9": [
    "None. The 9 is reached on the current evidence for the scoped CLI-only source merge. No code change is required."
  ],
  "evidence_read": [
    "docs/gateway-reliability/release-9.1.0/owner-merge-decision.json",
    "docs/gateway-reliability/INTEGRATION_9_1_0.md",
    "Verifier JSON output",
    "I did not open the individual source manifests, the 19 raw logs or qualification.json beyond what the verifier checks. My assessment of those relies on the verifier's hash and consistency checks plus the integration doc's description of them."
  ],
  "independent_verification": "I ran only the fixed read-only verifier, directly with no wrapper. It returned hashes_verified:true, cli_affected_checks_passed:true, workspace_all_targets_clippy_passed:true, mcp_full_suite_passed:false, mcp_mac_storage_refusals_per_compiler:56, source_files_verified:379, logs_verified:19, registry_pin_verified:'0.2.6', candidate_version:'9.1.0' and owner_accepted_cli_only_source_merge:true. This was receipt and hash verification. I did not execute any tests, and I make no claim of a green full workspace.",
  "remaining_limitations": [
    "The MCP service suite fails 56 tests per compiler at the Linux filesystem guard on macOS. Linux MCP qualification is deferred.",
    "No MCP deployment, Linux service acceptance or CLI distribution publication is authorized by this merge.",
    "Other platforms and the fleet, the M7 old-binary integration, live service contracts and CLI error-bridge field presentation remain pending and were not assumed to pass.",
    "One root helper and four opt-in history scale and memory cases are ignored, were not executed and are explained in the docs.",
    "Clippy was run on Rust 1.99 only.",
    "I did not read the source manifests, the raw logs or qualification.json directly, so I did not independently confirm that the four manifests pin the SDK and that five crates are at 9.1.0 beyond the verifier's output."
  ]
}
```
