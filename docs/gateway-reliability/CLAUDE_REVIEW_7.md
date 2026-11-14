# Claude Code review, iteration 7

Score: **8.5/10**. Historical snapshot.

Model: `claude-sonnet-5-5`. Read-only reviewer; supplied native results.
All scoped hashes were verified unchanged after review.

## Structured result

```json
{
  "repository": "/Users/cdot/development/goshawks.io/prisma-airs-cli",
  "score": 8.5,
  "assessment": "Within its declared scope this is a sound change. I found no critical, high or medium defect, only two low items. Body preparation and tenant binding now live in one pure module (request.rs). Execution (connect, provider resolution) and dry-run preview use that one module, and the tenant-binding checks keep their old order and messages. Preflight still uses the '0' placeholder, and the explicit and body organisation checks and the default-organisation insertion are unchanged. The one new check is selected-tenant TSG format, run before client build. Preview serializes through the same typed serializer (credential_wire) that execution uses, then projects it through an allow-list. Everything not recognized is withheld, with a redacted flag, and the whole result also goes through the registered-secret scrubber. Dry-run skips confirmation, the terminal gate, client and OAuth construction, and credential-file reservation. The `--secret-output` path is still validated and never reserved. --force conflicts with --dry-run at the clap level. The debug lifecycle event carries only static fields. Its operation name comes from a closed enum mapping, and secret-matching lines are dropped by the same check the transport records use. Regression coverage is strong: the incident reproductions, key-type input precedence, redaction, tenant refusals at the preflight, binding and provider-resolution checkpoints, rotation preview, the no-management-credentials case, and the unit tests for request, preview and debug. The pinned scm-sdk 0.2.4 matches what the code assumes: the key-create request has required `type` and `organisation_id`, and the deployment update uses typed auth settings. The implementer-supplied logs show passes on 1.89 and 1.99 for the integration (79), gateway unit (86), debug unit (14) and reference (2) suites. Clippy was run on 1.99 only. Remaining points are low-severity ergonomics and limits in the evidence, not correctness defects.",
  "findings": [
    {
      "severity": "low",
      "title": "Dry-run for a tenant with no mgmtTsgId fails with an unhelpful 'Invalid tsgId: ' error",
      "path": "src/gateway/request.rs",
      "line": 76,
      "evidence": "gateway_local_context (src/commands/gateway.rs:677) takes the selected TSG from `effective_str(\"mgmtTsgId\").unwrap_or_default()`. Preview deliberately skips require_management. Context::Tenant then calls check_tsg(\"tsgId\", selected). For a tenant with no mgmtTsgId (for example a scan-only tenant), selected is \"\" and the user gets `AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid tsgId: ` with exit 2. A tenant_binding debug event is also logged. Execution reaches require_management first and gives the clear 'requires nonempty mgmtTsgId, mgmtClientId\u2026' message. The no-management-credentials test (reliability.rs:430) removes only mgmtClientId and mgmtClientSecret and keeps the TSG, so this case is untested.",
      "impact": "Cosmetic, not a security or data problem. Preview refuses correctly, but the message does not tell the user to configure the tenant's TSG. The docs say management credentials are not required for preview, which is true of credentials but not of the TSG.",
      "recommendation": "Not a blocker. Either report a missing mgmtTsgId explicitly in preview, or add a test and doc sentence stating that preview still needs a configured mgmtTsgId."
    },
    {
      "severity": "low",
      "title": "A lifecycle debug event is silently dropped when a registered secret matches any of its static text",
      "path": "crates/airs-runtime-core/src/debug.rs",
      "line": 175,
      "evidence": "gateway_preparation_rejected builds the whole record line, including the keys, the 'validation_failure' event name and the operation, stage and checkpoint strings. It writes the line only if `appears_in_ignoring_case` finds no registered secret in it. If a short or common registered secret (for example a low-entropy word appearing in a key or event name) matches, the event is dropped with no warning and the log may stay empty. The unit test debug.rs:145 asserts this skip with no warning, so it is intended behaviour and not a leak.",
      "impact": "This fails safe for confidentiality. It can lose the diagnostic the change was added to provide, and the loss is invisible. It uses the same whole-line check as the existing transport records, so it is consistent with them, and only a pathological secret triggers it.",
      "recommendation": "Acceptable as is. A future refinement could drop the keys and fixed names from the check, or emit a body-free fallback event when one is skipped."
    }
  ],
  "verification": {
    "executed": [
      "Read-only static review with Read/Grep/Glob only. I executed no commands, builds or tests.",
      "Read in full: scope.md, manifest.json, NATIVE_CHECKS.md, native-checks/evidence.json, native-checks/vulture-preview-receipt.json and the full tracked diff patch.",
      "Read in full: src/gateway/request.rs (with its 6 unit tests), src/gateway/preview.rs (with its 3 unit tests) and tests/gateway_writes/reliability.rs (all tests).",
      "Read from the diff and the repo: crates/airs-runtime-core/src/debug.rs (new enums, gateway_preparation_rejected, the 2 direct debug tests), src/cli.rs, src/commands/gateway.rs (gateway_local_context), gateway_write_plan.rs (dry_run plumbing, destination()), gateway_writes.rs (run, prepare_request, preview, connect, authorise), src/gateway/writes.rs (diagnostic_operation, Typed::parse, credential_wire), src/gateway/mod.rs (check_tsg, foreign_tsg, shown) and commands::nothing_changed.",
      "Read: generated reference gateway.md diff, the guide section, COMPATIBILITY and DESIGN amendments, IMPLEMENTATION_PROGRESS.md.",
      "Checked the pinned scm-sdk-0.2.4 generated models in the cargo registry: the service and user key-create requests have required `type`, `organisation_id` and `workspace_id`, and the deployment update uses typed auth settings.",
      "Implementer-supplied logs I reviewed, not tests I ran: evidence.json and the log result lines show exit 0 and passes on macOS ARM64 on both Rust 1.99.0 and 1.89.0. Integration gateway_writes: 79 passed. Gateway unit: 86 passed. Debug unit: 14 passed. cli_reference: 2 passed. Clippy -D warnings on 1.99.0 (log read, no warnings).",
      "The tested-source SHA-256 values in evidence.json match the manifest hashes for all scoped source files that I compared. I compared the recorded values by eye and did not hash the files myself.",
      "The vulture preview receipt (implementer-supplied) records exit 0, locally_validated true, request_sent false, service_acceptance_checked false, 0 stderr bytes, and the candidate binary hash matches the 1.99.0 entry in evidence.json. It contains no credential values."
    ],
    "not_executed": [
      "I personally ran no cargo build, test, clippy, formatting or doc-link check, and did not run the CLI.",
      "No live service, OAuth, real credential or config store access.",
      "I did not hash the on-disk files or the retained binaries.",
      "I did not review the SDK sibling repository's diagnostics candidate, which is outside this scope."
    ],
    "limitations": [
      "All native evidence is implementer-supplied and local to macOS ARM64. No other platform was exercised.",
      "Clippy ran on 1.99.0 only, not on the 1.89 MSRV.",
      "Every test selection ran with --test-threads=1. The notes say an earlier stack assumption was found and fixed, so default-parallel thread behaviour for the unit selections was not exercised.",
      "The vulture receipt records only a stdout hash and not the output content, so I could not verify the preview contents from it. It is a local dry-run, not service acceptance.",
      "The tests are synthetic and loopback fixtures. They do not establish the service's accepted key-type vocabulary or deployment replacement/merge semantics. The tests and docs say this explicitly.",
      "Rustup's shared launcher was auto-updated from 1.29.1 to 1.29.3 as a disclosed tooling side effect.",
      "Whether the working-tree files still hash to the recorded values rests on the manifest, not on my own check."
    ]
  },
  "deferred_work": [
    "Adoption of the SDK diagnostics getter through the CLI error bridge. The CLI still uses the exact scm-sdk 0.2.4 pin and its existing generic schema messages (G3.1).",
    "Service contract evidence (G0.3): the actual service key-type vocabulary, organisation-response mismatch and deployment auth-settings replacement/merge behaviour.",
    "Other-platform checks and Clippy on the MSRV.",
    "Release, publishing, deployment and fleet qualification. None is claimed and no dependency or publishing change is authorized.",
    "Full lifecycle telemetry for file, credential, confirmation, malformed-input and missing-config refusals. These remain stderr-only, as the docs state.",
    "A tenant with no mgmtTsgId has no clear preview-specific message (low finding 1)."
  ],
  "verdict": "No critical, high or medium defects found in the scoped change. The two low items are an unhelpful preview error for a tenant with no TSG and an unreported drop of a debug event on a secret match. The score of 8.5 is for this source change's correctness, architecture, security and regression coverage only. It is not release approval. Native test evidence comes from implementer-supplied logs on macOS ARM64 for both Rust 1.89 and 1.99, which I reviewed and did not re-run. Getter adoption, the service contracts and release readiness remain deferred."
}
```

## Snapshot bindings

```json
{
  "repository": "/Users/cdot/development/goshawks.io/prisma-airs-cli",
  "head": "e235a4d0aa431eb0929c129f0ae0f6d008b07d7c",
  "files": {
    "crates/airs-runtime-core/src/debug.rs": "0961b844ef82ad5dcd30ce121fbd9928fb66fe4f99e4eb80f8b37cf7aee08bfd",
    "src/cli.rs": "4431de23f4e185caac493bf807ccf10dd1feda47b4ff62e5eaea226d09522f78",
    "src/commands/gateway.rs": "6c2274c628dca727a4575f7ae00083126423aeaa13d13881da0c6ab8352eda2d",
    "src/commands/gateway_write_plan.rs": "47f4c77e94655f83514730545f854c5ab57e602413e45e5a547de5c7abfb8513",
    "src/commands/gateway_writes.rs": "37de3c60aca39973d7d951b92c699304edcfc109d17594cd3c894ab3529ea936",
    "src/gateway/mod.rs": "14bc95a1175514468d0daaa641067e0a6ac32dd131c39f282da82db8a674284e",
    "src/gateway/request.rs": "b32992105006514f4395cbc32e59e989a0055e8d009a5800308aa44d5936086f",
    "src/gateway/preview.rs": "068c2bd2ffb8849885e20b0f40459ccf828c66b116799d06faa132576ce455f4",
    "tests/gateway_writes.rs": "898ea1501c6975db8e22d74bfbf7132a657a4176362f597d3c3f22cfd90f20ee",
    "tests/gateway_writes/reliability.rs": "805d5271fe878a511c240aa3e3e7155992d6fcd77a74b7b3b82537d213f44bd9",
    "docs-site/docs/commands/gateway.md": "19127a3a21c04119f7c48a90c07ad14e7e95db8e6d675b1266bc878ff18116f9",
    "docs/COMPATIBILITY.md": "13d415e1f870c9e580dd833ae2d68083d563772cf06f012ea7e1397bbe44397e",
    "docs/DESIGN.md": "4b2e63aa6a937dae1978147b7c61ca782f521ad40b69bb4743ab2ca168c2e1b7",
    "docs/GATEWAY_RELIABILITY_IMPLEMENTATION_PLAN.md": "27c97386d8da6960d297aae65f277b87d0b0ff9cdb5893f6d7a24dc97d49521d",
    "docs/gateway-reliability/G0_EVIDENCE.md": "d36162c11ff3629a51628c85578919917de7239b1b50e1a7f6c9470965717e81",
    "docs/gateway-reliability/IMPLEMENTATION_PROGRESS.md": "243c53fc4323d3332be2e00bd8fd61a970a334aaca614039ede101e6205c3574",
    "src/gateway/writes.rs": "67ec369eab9ae65a41a6d7ad1cc67ccadd6bb1796281e5a265d9bbd26a6be624",
    "docs-site/docs/reference/commands/gateway.md": "1835e4340245fd3330baa86c2a0f98c3a482dd3a7736a2c2cc37be9aa47b76f0",
    "docs/gateway-reliability/NATIVE_CHECKS.md": "cc9570a9c2b42c57c024385c799a4c818bccb7f31b9b2ae8a033cd42300b71cb",
    "docs/gateway-reliability/native-checks/evidence.json": "a13e256026d62d97246a3d0b9f89d5c7ab6adb4101a2c797d0c5b90e5c67f9b0",
    "docs/gateway-reliability/native-checks/cli-reference-check.log": "955ada51b8f5a8522163676e34a597296a126168bc3c9a4f1977b2d646fbe2ff",
    "docs/gateway-reliability/native-checks/cli-gateway-unit.log": "aa54e2aab32bcc6be7569e2bf062462fca416376cdbbedde989c54fcb9b04777",
    "docs/gateway-reliability/native-checks/cli-debug-unit.log": "538333b4f4429d2865253709055cf05292eeb9bea8698ce7fea42c2722d8453f",
    "docs/gateway-reliability/native-checks/cli-reference-generate.log": "9073b67897303981b3e503983f15fb0fd6f8203278bd7a59101a04df370361ad",
    "docs/gateway-reliability/native-checks/cli-clippy.log": "6d23fe6d09c151d2d0e1f093378266ab16d13e15deb0878405f92fb8333f459e",
    "docs/gateway-reliability/native-checks/cli-gateway-writes.log": "33747476a3e79f30b99fb119fe29894d0ce4f587e32af5fc06e4eba318d0dd5f",
    "docs/gateway-reliability/native-checks/cli-msrv-debug.log": "3eb49a02bc62a00ef608ae1be2507e8096aaa4b2c61349ac3fa4d872fa239cf0",
    "docs/gateway-reliability/native-checks/cli-msrv-unit.log": "3a422cb247711a09089b45623b6f3d546fe81795f792f709944addd29e1f11bf",
    "docs/gateway-reliability/native-checks/cli-msrv-gateway.log": "f9f9659d53bb78acad4081f9cb76d52c1680984808d6f6f0711acec1fb93645a",
    "docs/gateway-reliability/native-checks/vulture-preview-receipt.json": "0cec551e0596a16fd9cd8e8a75797a323bb61e8f49fb7703a3901069b582621f",
    "docs/gateway-reliability/native-checks/cli-msrv-reference.log": "e3a2e8e38104d7f65209fa017f96db4bfcf94b945c9f2a1f2b05df004c8c8fd7"
  }
}
```
