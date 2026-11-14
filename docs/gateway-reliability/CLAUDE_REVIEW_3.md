# Claude Code source review, iteration 3

Score: **8/10**. Historical snapshot; later changes need another review.

Actual model: `claude-sonnet-5-5`; read-only Read/Grep/Glob, no MCP.
No native build/tests, services or credential stores were accessed.
All scoped hashes were verified unchanged after review.

## Structured result

```json
{
  "repository": "/Users/cdot/development/goshawks.io/prisma-airs-cli",
  "score": 8,
  "assessment": "This is a source-only review: nothing was compiled or run. I read all the required evidence and found no compile, type, feature or borrow errors, no redaction leak, and no acceptance or normalization regression. The refactor moves organisation binding and typed validation into a pure `PreparedRequest`. Execution and preview share it. The ordering is preserved: `require_management` still runs before validation, preflight uses placeholder organisation \"0\", and provider resolution re-prepares with the same selected organisation and explicit flag. The `connect` rewrite keeps the old effective behavior. The tenant-flag and body-organisation refusals, the `--tsg-id` flag name for `AuthSettingsUpdate`, and the organisation default fill are all unchanged. `diagnostic_operation` covers every one of the 34 `Write` variants. The new debug event is static and body-free. The preview is an allow-list projection followed by the shared resource policy and `SecretSet` scrub. Dry-run never builds clients, authenticates, prompts, or reserves a file. The regression file does contain explicit tests for what the scope claimed. They cover foreign flag and body tenant refusals in both modes, logging at each stage and checkpoint, missing management credentials, force conflict, rotate without a destination, pretty output, quiet JSON, URL and label redaction, and the provider-resolution invalid-UUID case. Test helpers and fixtures resolve: `Fx`, `bare`, `fx`, `gw_with`, `debug_log::only_log`, `fx::marker`, `deployment_detail`, `add_fixture_tenant_with`, `Gw::CatalogProviders`, and `scm::validation::Mode` with `GatewayDeploymentUpdateRequest::parse`. The generated-parser evidence agrees with the test's `parse(value, Mode)` use. I found two small residual defects: a preview/execution validation divergence on a malformed tenant TSG, and a unit-test gap for the new debug primitive. Both are low severity. Native build and test remain unexecuted, so the score reflects code reading only.",
  "findings": [
    {
      "severity": "low",
      "title": "Preview applies a tenant-TSG format check that execution does not",
      "path": "src/commands/gateway_writes.rs",
      "line": 216,
      "evidence": "`preview()` calls `gateway::check_tsg(\"tsgId\", &tsg)` on the tenant's `mgmtTsgId` before `prepare_request`. `connect()` (execution) calls `gateway_local_context`, `require_management` and `prepare_request` without that check. `Invocation::gateway_clients` has none either. A grep shows `check_tsg` is used only for flag values and inventory, and the preview is its only use on the tenant's own TSG.",
      "impact": "A tenant whose stored `mgmtTsgId` is malformed would make the preview exit with an error that normal execution does not raise at that point. That weakens the claim that preview and execution share one preparation path. Preview is the stricter of the two, so this is not unsafe. The refusal is also not logged as a lifecycle event.",
      "recommendation": "Either apply the same TSG check in `connect` or in the shared local-context helper, or document the preview-only check."
    },
    {
      "severity": "low",
      "title": "New debug lifecycle primitive has no direct unit test in its own crate",
      "path": "crates/airs-runtime-core/src/debug.rs",
      "line": 156,
      "evidence": "`gateway_preparation_rejected` is covered only by CLI integration tests in `tests/gateway_writes/reliability.rs`. The runtime-core diff adds no test for the record shape, the `SecretSet` skip branch, or the enum name mappings.",
      "impact": "The static field names and enum string mappings are covered only through the native CLI integration suite. A change to `debug.rs` could break them without a fast crate-level test failing.",
      "recommendation": "Add a crate-level test of the record shape and the skip branch."
    }
  ],
  "verification": {
    "executed": [
      "Read the scope index and the full tracked diff in all 727 lines, covering debug.rs, cli.rs, gateway.rs, gateway_write_plan.rs, gateway_writes.rs, gateway/mod.rs, gateway/writes.rs (Write identity), tests/gateway_writes.rs, and the docs-site, COMPATIBILITY and DESIGN amendments.",
      "Read src/gateway/request.rs in full (266 lines), src/gateway/preview.rs in full (296 lines) and tests/gateway_writes/reliability.rs in full (726 lines), in chunks of at most 200 lines.",
      "Read the full schema-evidence.json frozen graph and generated-parser-evidence.md, covering the deployment-update, service-key-create and auth-settings schemas, the SDK feature list and the parser signature.",
      "Read docs/gateway-reliability/IMPLEMENTATION_PROGRESS.md and grepped the plan's status text.",
      "Traced supporting code: Write enum and refine/Typed::parse/credential_wire/policy/route in writes.rs, KeyKind, gateway_clients and gateway_local_context, check_tsg, foreign_tsg, destination() and secret_write, nothing_changed, the withhold policies and apply(), OValue derives, render_detail, datetime_millis, the debug.rs helpers and re-exports, SecretSet::appears_in_ignoring_case, provider::resolve, the deployment fixtures and test-harness helpers, scm-sdk 0.2.4 module visibility, and the Cargo features and pinned SDK.",
      "Grepped for stale references to removed symbols (ORGANISATION_PLACEHOLDER, foreign_body_org, typed(), connected.body and connected.typed) and for struct-literal constructions of the changed argument and plan types. None remain."
    ],
    "not_executed": [
      "cargo build, check, clippy, test or MSRV.",
      "Any CLI, fixture-server or live service execution.",
      "SDK candidate tests and no-default-feature builds.",
      "Generated CLI reference regeneration and documentation checks.",
      "Service-contract validation (key-type vocabulary, deployment replacement semantics)."
    ],
    "limitations": [
      "Source reading only; type, borrow and test outcomes are inferred rather than compiled.",
      "The fixture server's catalog tolerance and the exact SDK serializer output for the deployment preview were not executed. I confirmed that the `DEPLOYMENTS` policy has no paths and that `provider::resolve` does not itself check UUIDs.",
      "I did not read the full SDK sibling working tree or its diagnostics changes. The CLI intentionally stays on the exact SDK 0.2.4 pin.",
      "Review and scoring cover the CLI source only. AI-Red-Teaming-bulk-scan.csv and brief/ were excluded."
    ]
  },
  "deferred_work": [
    "Native build and run of `reliability::` and the existing gateway write, confirmation, provider-resolution and credential-preservation regressions.",
    "SDK getter and diagnostic adoption through the shared error bridge, with an immutable published dependency.",
    "G0.3 service-contract evidence: key-type vocabulary, the actual organisation-response mismatch, and deployment replacement versus merge semantics.",
    "Regenerating the CLI reference and executing the documentation checks.",
    "Non-blocking context: the preview's hard-coded known-literal lists for type and scope (`preview.rs:134`, `preview.rs:155`), and the partial duplication of `gateway_clients` logic inside `connect`, will need maintenance if vocabularies or the client-construction path change. They are documented and not current bugs."
  ],
  "verdict": "Source-review-only: no critical, high or medium defects found; two low findings (a preview-only TSG check, and a missing crate-level debug-event test). Score 8/10 for the source, with native acceptance still required. This is not a release decision."
}
```

## Snapshot bindings

```json
{
  "repository": "/Users/cdot/development/goshawks.io/prisma-airs-cli",
  "head": "e235a4d0aa431eb0929c129f0ae0f6d008b07d7c",
  "files": {
    "crates/airs-runtime-core/src/debug.rs": "d8756ca1970dce84a2aac2851d8e0c03b1bbcced3c084d712d7253f2472deceb",
    "src/cli.rs": "4431de23f4e185caac493bf807ccf10dd1feda47b4ff62e5eaea226d09522f78",
    "src/commands/gateway.rs": "6c2274c628dca727a4575f7ae00083126423aeaa13d13881da0c6ab8352eda2d",
    "src/commands/gateway_write_plan.rs": "47f4c77e94655f83514730545f854c5ab57e602413e45e5a547de5c7abfb8513",
    "src/commands/gateway_writes.rs": "87807dd3b6beb9143451503811ca92970fae897d936e075158a10e3223c3d9e7",
    "src/gateway/mod.rs": "14bc95a1175514468d0daaa641067e0a6ac32dd131c39f282da82db8a674284e",
    "src/gateway/request.rs": "283eec1255f4fd3ef4fd3bddb81b6f0d27feec2e87b72e3aa5cbc9023dc77edc",
    "src/gateway/preview.rs": "068c2bd2ffb8849885e20b0f40459ccf828c66b116799d06faa132576ce455f4",
    "tests/gateway_writes.rs": "898ea1501c6975db8e22d74bfbf7132a657a4176362f597d3c3f22cfd90f20ee",
    "tests/gateway_writes/reliability.rs": "718aba25710960e68703460c9e0ecc4bb6ecf489dae17b5f429b2df393baf633",
    "docs-site/docs/commands/gateway.md": "c7db8cd5219c6827ce10098e2e44d45fbf74f0c5f2fb4915a3e87424ee670762",
    "docs/COMPATIBILITY.md": "13d415e1f870c9e580dd833ae2d68083d563772cf06f012ea7e1397bbe44397e",
    "docs/DESIGN.md": "fbba4ae1fd4a74333ad92d5795ceb38cd77851495c417caf507705f68777ac81",
    "docs/GATEWAY_RELIABILITY_IMPLEMENTATION_PLAN.md": "793ada20a7c64ff990acdefd5718d62129fa897a5f8811cc39bc69d8071f4aab",
    "docs/gateway-reliability/G0_EVIDENCE.md": "d36162c11ff3629a51628c85578919917de7239b1b50e1a7f6c9470965717e81",
    "docs/gateway-reliability/IMPLEMENTATION_PROGRESS.md": "db26185a44e75e5b1dce9155cc4f5603fb8f9c913fb9ba559bb20bae3230360d",
    "src/gateway/writes.rs": "67ec369eab9ae65a41a6d7ad1cc67ccadd6bb1796281e5a265d9bbd26a6be624"
  }
}
```
