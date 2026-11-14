# Claude Code source review, iteration 4

Score: **8/10**. Historical snapshot; later changes need another review.

Actual model: `claude-sonnet-5-5`; read-only Read/Grep/Glob, no MCP.
No native build/tests, services or credential stores were accessed.
All scoped hashes were verified unchanged after review.

## Structured result

```json
{
  "repository": "/Users/cdot/development/goshawks.io/prisma-airs-cli",
  "score": 8,
  "assessment": "Source-review-only; nothing was compiled or run. I found no critical, high or medium defect in the iteration-4 scope. The prior low findings are fixed in source. The selected-tenant TSG format check now sits in `PreparedRequest::prepare` under `Context::Tenant` (request.rs:76) and is shared by execution (`connect`), preview and provider resolution through one logging adapter, `prepare_request`. Runtime-core now has direct tests for the debug event's exact shape, enum names, case-insensitive secret suppression and continued logging. The design is sound. The preview is built from the typed serializer, `credential_wire`, so it matches what execution sends. Dry-run has no client, token, confirmation prompt or reservation path, and `destination(.., !dry_run)` only relaxes the 'required' check, not the existence check. I confirmed from the code that the preflight/binding/provider-resolution checkpoints each log at most once per refusal. Execution ordering is unchanged apart from the body-tenant check now running before client construction, which is safe. I found only one small, low-severity inconsistency between the preview projection and its documentation. The score is held at 8 because every test is unexecuted, the SDK diagnostic getter is deliberately not adopted, and service-contract acceptance (key `type` vocabulary, deployment replace/merge semantics) is unestablished. Those are deferred or unauthorized work, not defects.",
  "findings": [
    {
      "severity": "low",
      "title": "Preview shows `ws-`-prefixed workspace slugs in clear; docs only describe type/scope literals as shown",
      "path": "src/gateway/preview.rs",
      "line": 61,
      "evidence": "`auth_settings` copies any `workspaces_allowed` / `remove_workspaces_allowed` string that starts with `ws-`, is at most 128 bytes and is alphanumeric, `-` or `_`. docs-site gateway.md says 'Preview displays only known type and scope literals. Other valid strings appear as [withheld]'. DESIGN.md says 'arbitrary configuration, credentials, labels and dynamic keys are withheld'. The `write_shaped_deployment_settings...` and `deployment_dry_run_and_execution_share_the_effective_request` tests assert that `ws-one` is shown. No test covers a `ws-` slug that encodes a customer or project name.",
      "impact": "A user-chosen identifier-like string appears unredacted in preview output, and the 'known literals only' wording is not literally true. Workspace slugs also appear in routes and the existing deployment views, so this is unlikely to be a secret leak. It is a documentation/policy mismatch, and the output is still passed through the SecretSet scrub.",
      "recommendation": "State in the docs and design amendment that well-formed `ws-` workspace slugs, organisation IDs, workspace UUIDs and valid timestamps are shown. Alternatively, withhold the slugs and update the tests."
    }
  ],
  "verification": {
    "executed": [
      "Read the scope index prisma-airs-cli-scope.md, manifest.json, schema-evidence.json (full frozen graph: GatewayDeploymentUpdateRequestSchema s006508 with strict auth_settings s006470, GatewayServiceApiKeyCreateRequestSchema s006300 requiring type, scopes with min 1, uuid workspace_id and numeric-TSG organisation_id, the datetime check allowing offset and fractional seconds) and generated-parser-evidence.md (GatewayDeploymentUpdateRequest::parse via support::parse, feature list).",
      "Read src/gateway/request.rs in full (285 lines, in two chunks), including its 7 unit tests.",
      "Read src/gateway/preview.rs in full (296 lines, in two chunks), including its 3 unit tests.",
      "Read tests/gateway_writes/reliability.rs in full (759 lines, in four chunks). Confirmed by reading that they exist: foreign flag and body tenant refusals in both execution and dry-run (`execution_and_preview_log_flag_and_body_tenant_refusals_once`, plus `dry_run_does_not_bypass_schema_or_tenant_validation`), stage/checkpoint logging, missing management credentials, force conflict, rotating preview without a destination, pretty output, quiet JSON, URL/label withholding, the provider-resolution invalid-UUID test, and the shared selected-tenant-format test.",
      "Read the tracked diff patch (817 lines in full). It covers debug.rs including the new unit tests at its end, the CLI args, gateway.rs `gateway_local_context`, gateway_write_plan.rs, gateway_writes.rs (run, prepare, preview, connect, authorise, send_typed), gateway/mod.rs, Write::diagnostic_operation in writes.rs, tests/gateway_writes.rs, gateway.md, COMPATIBILITY.md and DESIGN.md.",
      "Read docs/gateway-reliability/IMPLEMENTATION_PROGRESS.md in full (status, remediation, pending work, explicit statement that nothing was run natively).",
      "Traced supporting code: `check_tsg`, `foreign_tsg` and `shown` in gateway/mod.rs; `gateway_clients` and `gateway_local_context` in gateway.rs; `destination` and `secret_write` in gateway_write_plan.rs; `plan_destination` in gateway/secrets.rs; `Typed::parse` and `credential_wire` in writes.rs; `Policy` and `Path` in withhold.rs; `datetime_millis`; `SecretSet` re-exports and `appears_in_ignoring_case`; `DebugLog::write_line` and `line`; `OValue::Number`; the test-harness helpers (`only_log`, `xdg_state_home`, `RunOptions::env`, `Fx` helpers and imports in tests/gateway_writes.rs).",
      "Checked symbols and imports used by the new code (`crate::modelsec::datetime_millis` is pub(crate), `PreparationError: Debug` for the test `unwrap`s, `Write: PartialEq`) against the sources I read. I found no compile or type problem.",
      "Checked sample redaction behavior by reading the projection logic. `name`, `description`, `defaults`, `rate_limits`, `alert_emails` and `tags` are withheld; `type`, `status` and `scopes` are shown only for known literals; `gateway_base_url`, JWT mappings and numeric `allow_all_workspaces` are withheld; unknown keys are dropped."
    ],
    "not_executed": [
      "No build, cargo, rustfmt, tests, native binary, services or live endpoints. All tests are unexecuted.",
      "I did not read crates/scm-sdk files from the SDK manifest (engine, diagnostic, validation). They are out of this CLI review's scope and the CLI stays on SDK 0.2.4.",
      "I did not read the full G0_EVIDENCE.md, the plan body or the previous review JSON beyond what the scope index summarizes. I read IMPLEMENTATION_PROGRESS.md for status.",
      "AI-Red-Teaming-bulk-scan.csv and brief/ are excluded as instructed. No credential, tenant or configuration stores were read."
    ],
    "limitations": [
      "A source-only read cannot prove compilation, MSRV 1.89 / edition2024 or let-chain compatibility, or runtime test behavior. This includes the debug-log tests that assume an empty-but-present log file for a valid dry-run, and the case-folded secret-suppression assumption in `appears_in_ignoring_case`.",
      "Fixture assumptions (the provider-catalog row edit, `single_tenant` with an invalid TSG exiting with code 2, rotation preview behavior) are verified by reading only, not by execution.",
      "The accepted key `type` vocabulary and deployment replace/merge semantics are unestablished service contracts. The preview's known-literal list (`production`, `non_production`, `workspace-service`, `service`) is a local policy and not service evidence.",
      "This is a source-implementation score, not a release decision and not native acceptance."
    ]
  },
  "deferred_work": [
    "Native build and test execution in an admitted environment: the reliability selection, existing gateway write/confirmation/provider/credential regressions, debug.rs unit tests and the rustc/MSRV check.",
    "SDK diagnostic getter adoption through the runtime-core error bridge (G3.1) so CLI stderr and lifecycle events can carry field/stage facts; final exact-pin adoption across consumers once an immutable SDK exists.",
    "G0.3 service evidence for the organisation-response mismatch, key type vocabulary, and deployment replace/merge behavior.",
    "Regenerate the CLI reference from the candidate executable and record binary hashes and phase acceptance.",
    "Optionally extend lifecycle telemetry to file, credential, confirmation and malformed-input refusals; the docs say these are deliberately stderr-only."
  ],
  "verdict": "Source-review-only: no critical, high or medium issues found. One low documentation/policy inconsistency (the preview's `ws-` slugs). Prior low findings are fixed in source. Score 8/10. Native acceptance is still required, so this is not a release decision."
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
    "src/commands/gateway_writes.rs": "93319821c7c58139a751e86a3ba29cbba15a83f310547472b90634f7be295e1a",
    "src/gateway/mod.rs": "14bc95a1175514468d0daaa641067e0a6ac32dd131c39f282da82db8a674284e",
    "src/gateway/request.rs": "b32992105006514f4395cbc32e59e989a0055e8d009a5800308aa44d5936086f",
    "src/gateway/preview.rs": "068c2bd2ffb8849885e20b0f40459ccf828c66b116799d06faa132576ce455f4",
    "tests/gateway_writes.rs": "898ea1501c6975db8e22d74bfbf7132a657a4176362f597d3c3f22cfd90f20ee",
    "tests/gateway_writes/reliability.rs": "805d5271fe878a511c240aa3e3e7155992d6fcd77a74b7b3b82537d213f44bd9",
    "docs-site/docs/commands/gateway.md": "c7db8cd5219c6827ce10098e2e44d45fbf74f0c5f2fb4915a3e87424ee670762",
    "docs/COMPATIBILITY.md": "13d415e1f870c9e580dd833ae2d68083d563772cf06f012ea7e1397bbe44397e",
    "docs/DESIGN.md": "fbba4ae1fd4a74333ad92d5795ceb38cd77851495c417caf507705f68777ac81",
    "docs/GATEWAY_RELIABILITY_IMPLEMENTATION_PLAN.md": "793ada20a7c64ff990acdefd5718d62129fa897a5f8811cc39bc69d8071f4aab",
    "docs/gateway-reliability/G0_EVIDENCE.md": "d36162c11ff3629a51628c85578919917de7239b1b50e1a7f6c9470965717e81",
    "docs/gateway-reliability/IMPLEMENTATION_PROGRESS.md": "27ae88ee0587cc937391c2aaf73cd320981675763467509a278094845d1ae436",
    "src/gateway/writes.rs": "67ec369eab9ae65a41a6d7ad1cc67ccadd6bb1796281e5a265d9bbd26a6be624"
  }
}
```
