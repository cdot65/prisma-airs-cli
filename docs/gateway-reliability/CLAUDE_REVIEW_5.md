# Claude Code source review, iteration 5

Score: **8/10**. Historical snapshot, before native execution results.

Actual model: `claude-sonnet-5-5`; read-only Read/Grep/Glob, no MCP.
The reviewer did not build or run code. Native execution began separately after
the owner supplied a tenant environment. The SDK synthetic deep-path fixture
changed afterward to use an explicit worker stack; a fresh review is required.

## Structured result

```json
{
  "repository": "/Users/cdot/development/goshawks.io/prisma-airs-cli",
  "score": 8,
  "assessment": "Source review only. Nothing was compiled or run, and no native test has executed. I found no critical, high, medium or low defect in the current scope. The one low finding from iteration 4 is resolved. docs-site gateway.md and DESIGN.md now list what the preview displays: known type/status/scope literals, numeric organisation IDs, workspace UUIDs, valid expiry timestamps and well-formed `ws-` slugs. Both docs warn that slugs can identify a project or customer, and they limit the known-literal policy to type/status/scope values. This matches the `preview.rs` projection (`auth_settings` at lines 38-92, `body_view` at 94-178). The design is sound. `PreparedRequest::prepare` is pure and does selected-tenant TSG format checking, flag and body tenant mismatch checks, organisation defaulting and strict typed parsing. `prepare_request` in gateway_writes.rs is the one logging adapter for the preflight, tenant-binding and provider-resolution checkpoints. The preview is built from `Typed::credential_wire`, the same serializer execution uses. The dry-run path (`run` -> `preview`) returns before `connect`, `confirmed` and `authorise`. It builds no client, requests no token, shows no prompt and reserves no destination. Dry-run also skips the confirm terminal gate. `destination(.., !dry_run)` only relaxes the 'required' check. The existing-file check still runs, and the key dry-run test asserts exit 1, no request and an untouched file. Execution ordering is preserved. Tenant load and `require_management` come first, then binding and body-tenant checks, then `GatewayClients::build`. Both foreign-flag naming cases are kept (`--tsg-id` for `AuthSettingsUpdate`). The debug event carries only static, enum-derived fields and is skipped, not fatal, on a case-insensitive secret match. I read the tests and confirmed the regressions the scope names. I found no compile, type or feature problem by reading. The score stays at 8 because every test is unexecuted, SDK diagnostic getter adoption is deliberately deferred, and the service contracts (key `type` vocabulary, deployment replace/merge semantics) are not established. Those are deferred or unauthorized work, not defects.",
  "findings": [],
  "verification": {
    "executed": [
      "Read prisma-airs-cli-scope.md, manifest.json and the previous review JSON (iteration 4: score 8, one low preview-docs finding).",
      "Read schema-evidence.json in full. It holds GatewayDeploymentUpdateRequestSchema s006508 with strict auth_settings s006470 (numeric-0/1 literal unions for the hosted/playground flags, boolean allow_all_workspaces, and `disable_portkey_gateway` and `grouped_gateway_base_url` absent). It also holds GatewayServiceApiKeyCreateRequestSchema s006300 (required nonempty `type`, scopes min 1, uuid workspace_id, numeric organisation_id) and the datetime check allowing offset and fractional seconds. Checked against the request.rs/preview.rs tests and the reliability.rs expectations.",
      "Read generated-parser-evidence.md (GatewayDeploymentUpdateRequest::parse via support::parse with SCHEMA s006508, and the SDK feature list).",
      "Read src/gateway/request.rs in full (285 lines, two chunks), including its unit tests for preflight vs binding, foreign body/flag tenants, an explicit matching organisation, the deployment schema, error classification and redacted Debug, and the shared selected-tenant-format check.",
      "Read src/gateway/preview.rs in full (296 lines, two chunks), including its three projection tests.",
      "Read tests/gateway_writes/reliability.rs in full (758 lines, four chunks). Confirmed by reading the foreign flag and body tenant refusals in both execution and dry-run, once-only logging, stage/checkpoint assertions, missing management credentials, force conflict, rotate preview without a destination, pretty and quiet-JSON URL/label withholding, the provider-resolution invalid-UUID test (one event, only the provider GET sent, no writes), and the shared selected-tenant-format test.",
      "Read the full tracked diff (825 lines), covering debug.rs including the direct unit tests at its end, cli.rs args, gateway.rs `gateway_local_context`, gateway_write_plan.rs, gateway_writes.rs (run, prepare_request, prepare, preview, connect, authorise, send_typed), gateway/mod.rs, `Write::diagnostic_operation`, tests/gateway_writes.rs, gateway.md, COMPATIBILITY.md and DESIGN.md.",
      "Read docs/gateway-reliability/IMPLEMENTATION_PROGRESS.md in full, including the iteration 5 remediation notes and the explicit statement that nothing was run natively.",
      "Traced supporting code: `check_tsg`, `foreign_tsg` and `shown` in gateway/mod.rs; `gateway_clients` and `gateway_local_context` in commands/gateway.rs; `Typed::credential_wire` in writes.rs; and the existence and signatures of `Write::policy`, `route`, `plane`, `WritePlan::secret_write` and `modelsec::datetime_millis` (pub(crate)). No symbol was missing."
    ],
    "not_executed": [
      "No cargo, rustc, rustfmt, build or test run, and no native binary, service or live endpoint. All tests are unexecuted.",
      "The scm-sdk-rs files in the manifest (engine, diagnostic, validation) were not read. They are outside this CLI review, and the CLI stays pinned to SDK 0.2.4.",
      "The G0_EVIDENCE.md and plan body were not read in full. I read IMPLEMENTATION_PROGRESS.md for status.",
      "AI-Red-Teaming-bulk-scan.csv and brief/ were excluded as instructed. No credential, tenant or configuration store was read."
    ],
    "limitations": [
      "A source read cannot prove compilation, MSRV 1.89/edition2024 or let-chain compatibility, or runtime test outcomes. That includes the partial move out of `Prepared` in `preview`, which I believe is valid because the struct has no Drop impl. It also includes the debug-log tests that assume an empty-but-present log file after a valid dry-run, and the fixture mutations (catalog row edit, `single_tenant` with an invalid TSG exiting with code 2).",
      "The accepted key `type` vocabulary and deployment replace/merge behavior are unestablished service contracts. The preview's known-literal list is a local display policy, not service evidence.",
      "This is a candid source-implementation score, not native acceptance and not a release decision."
    ]
  },
  "deferred_work": [
    "Native build and test execution in an admitted environment: the reliability selection, existing gateway write/confirmation/provider/credential regressions, the runtime-core debug unit tests and the MSRV check.",
    "SDK diagnostic getter adoption through the runtime-core error bridge (G3.1) so CLI stderr and lifecycle events can carry field/stage facts, then exact-pin adoption once an immutable SDK exists.",
    "G0.3 service evidence for the organisation-response mismatch, the key type vocabulary and deployment replace/merge semantics.",
    "Regenerate the CLI reference from the candidate executable and record binary hashes and phase acceptance.",
    "Optionally extend lifecycle telemetry to file, credential, confirmation and malformed-input refusals. The docs state these are deliberately stderr-only."
  ],
  "verdict": "Source review only: no findings at any severity. The iteration-4 low documentation mismatch is fixed, and the prior fixes and regressions are present as described. Score 8/10, held back by entirely unexecuted tests and deferred SDK/service integration, not by a source defect. Native acceptance is still required; this is not a release decision."
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
    "docs-site/docs/commands/gateway.md": "19127a3a21c04119f7c48a90c07ad14e7e95db8e6d675b1266bc878ff18116f9",
    "docs/COMPATIBILITY.md": "13d415e1f870c9e580dd833ae2d68083d563772cf06f012ea7e1397bbe44397e",
    "docs/DESIGN.md": "4b2e63aa6a937dae1978147b7c61ca782f521ad40b69bb4743ab2ca168c2e1b7",
    "docs/GATEWAY_RELIABILITY_IMPLEMENTATION_PLAN.md": "793ada20a7c64ff990acdefd5718d62129fa897a5f8811cc39bc69d8071f4aab",
    "docs/gateway-reliability/G0_EVIDENCE.md": "d36162c11ff3629a51628c85578919917de7239b1b50e1a7f6c9470965717e81",
    "docs/gateway-reliability/IMPLEMENTATION_PROGRESS.md": "68efc8ad597faa5b4f3a05f930203a55ee00a60acead33fe11afdfd315603948",
    "src/gateway/writes.rs": "67ec369eab9ae65a41a6d7ad1cc67ccadd6bb1796281e5a265d9bbd26a6be624"
  }
}
```
