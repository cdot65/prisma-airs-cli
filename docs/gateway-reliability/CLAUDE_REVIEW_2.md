# Claude Code source review, iteration 2

Score: **7.5/10**. Historical snapshot; later changes require another review.

Actual Claude Code model: `claude-sonnet-5-5`. Read/Grep/Glob only;
no edit, build, test, service, credential or MCP access was permitted.
All scoped hashes were verified unchanged after review. This review is partial
where Claude explicitly reports unread evidence; it is not native acceptance.

## Structured result

```json
{
  "repository": "prisma-airs-cli",
  "score": 7.5,
  "assessment": "This is a source-review score only. No native build, test, or service check was run, so nothing here claims passing validation. The shared `PreparedRequest` module is a sound design. Execution and preview now go through the same typed validation, tenant binding and normalization, and the tenant and body-organisation checks keep their earlier semantics. The dry-run path makes no authentication or client call, does not reserve the credential file, and conflicts with `--force`. The preview is allow-list based, so I found no leak path in it. The new debug event is static and body-free, and its stage and checkpoint are classified at the source rather than by parsing error text. I traced the new symbols against existing code (`Typed::parse(&Value, Option<&str>)`, `Policy` pub fields, the `crate::debug` glob re-export, `modelsec::datetime_millis` as pub(crate), `model_value`, `render_detail`) and found no type or compile error. I found no critical or high defect. The remaining problems are documentation that overstates how much the debug event covers, a few unrecorded refusal paths, and gaps in regression coverage. The sibling-SDK 0.2.5 getter adoption is intentionally deferred and not counted against the score.",
  "findings": [
    {
      "severity": "medium",
      "title": "Docs say all write-preparation failures after debug-log creation are recorded, but only typed validation and tenant-binding refusals are",
      "path": "docs-site/docs/commands/gateway.md",
      "line": 213,
      "evidence": "The docs say 'Write-preparation failures after debug-log creation record a validation_failure lifecycle event', and DESIGN.md says 'Gateway preparation failures now also emit an application lifecycle record'. The only emitter is `prepare_request` (src/commands/gateway_writes.rs:135), which wraps `PreparedRequest::prepare`. Other refusals in the same phases return without a log record: NEEDS_DESTINATION and ONLY_ROTATE (gateway_writes.rs:178-180), the non-interactive `gate_on_terminals` refusal (:200), body-build and `--set` errors, `check_tsg` and `require_management` in `connect` and `preview`, and credential-source and destination-path errors. COMPATIBILITY.md is narrower ('local semantic write-validation failures').",
      "impact": "A user who runs `--debug` expecting an event for every preparation failure still gets an empty log for the unrecorded cases. That is the same empty-log symptom the change set out to fix, and the docs mislead consumers of the log.",
      "recommendation": "Either narrow the docs to typed-schema validation and tenant-binding refusals, or route the remaining preparation refusals through the same adapter with additional stage values."
    },
    {
      "severity": "low",
      "title": "Regression coverage for tenant-binding and provider-resolution checkpoints and for dry-run refusal paths is not shown",
      "path": "tests/gateway_writes/reliability.rs",
      "line": 248,
      "evidence": "The debug-event test only asserts stage `request_validation` and checkpoint `preflight`, for two operations. The unit tests in request.rs check `FailureKind` only. In the portion of reliability.rs I read, no test covers a dry-run with a foreign `--organisation-id`, a foreign body `organisation_id`, a missing management credential, `--dry-run` combined with `--force`, or the `tenant_binding` and `provider_resolution` log records. The `rotate_auth` dry-run without a destination is also not exercised in the portion I read.",
      "impact": "Part of the new behavior (the three-checkpoint adapter, the dry-run relaxation of NEEDS_DESTINATION, and the order of the tenant-binding check against `GatewayClients::build`) is not protected against regression.",
      "recommendation": "Add integration tests for the tenant-binding and provider-resolution checkpoints, and for the dry-run edge cases listed above."
    },
    {
      "severity": "low",
      "title": "Preview withholds any key `type` outside a four-value allow-list, while docs say the contract accepts any nonempty string",
      "path": "src/gateway/preview.rs",
      "line": 130,
      "evidence": "`body_view` shows `type` only for production, non_production, workspace-service and service, and otherwise returns `[withheld]`. The docs say 'The pinned local contract accepts a nonempty string; use the creation type documented for your service.' A legitimate service-defined type therefore previews as `[withheld]`. The `scopes` arm (:147) also turns a non-array value into an empty array without setting `redacted`, but typed validation makes that unreachable today.",
      "impact": "The operator cannot confirm in the preview the value that most often caused the original failure. `redacted` reports true in that case, so it is not a leak, only a loss of usefulness.",
      "recommendation": "Document the limitation, or show a bounded charset-checked type string. Set `redacted` in the non-array scopes branch for robustness."
    }
  ],
  "verification": {
    "executed": [
      "Static reading of the scope manifest, the tracked diffs, the new files request.rs, preview.rs and reliability.rs (partial), and the surrounding existing code (gateway_clients, check_tsg, foreign_tsg, Typed::parse, credential_wire, Policy, debug re-exports)."
    ],
    "not_executed": [
      "cargo build, clippy and tests",
      "Any native, integration or live-service check",
      "Verification against the sibling SDK 0.2.5 (the CLI remains pinned to scm-sdk 0.2.4)"
    ],
    "limitations": [
      "This is a source-review score only. Compile correctness is judged by reading, not by a compiler.",
      "I read only part of reliability.rs, and the tail of the scope file (lines 1758 onward, including docs and ledger content) was not reviewed.",
      "I did not independently check schema-evidence.json.",
      "I did not read the previous iteration's reports, so the resolution of earlier findings is judged only by whether current source contains concrete defects."
    ]
  },
  "deferred_work": [
    "Adopt the new scm-sdk getter and diagnostics through the shared error bridge (the CLI stays on 0.2.4).",
    "Native build and test execution in an admitted environment.",
    "Service-side acceptance of the key-type vocabulary and deployment replacement/merge semantics.",
    "Logging for argument-parser failures that occur before the debug log is created."
  ],
  "verdict": "Sound, well-structured source change with no critical or high defect found. The score is held back by overstated debug-event documentation, unrecorded refusal paths, partial regression coverage of the new checkpoints, and the absence of any executed validation. Do not treat this as native-validated."
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
    "src/gateway/preview.rs": "1693984101577adcaf6081b71f979535453cb3e39869290d15abbad6bcfb6545",
    "tests/gateway_writes.rs": "898ea1501c6975db8e22d74bfbf7132a657a4176362f597d3c3f22cfd90f20ee",
    "tests/gateway_writes/reliability.rs": "49fe7f2af44cc66e81f9abb96e589501433d029040a16f292a30baccef4e6421",
    "docs-site/docs/commands/gateway.md": "ea4a2eb4f8a8c5052d399e9dad451ccd2b326abf4befaa288f0b208a6ee317e3",
    "docs/COMPATIBILITY.md": "13d415e1f870c9e580dd833ae2d68083d563772cf06f012ea7e1397bbe44397e",
    "docs/DESIGN.md": "a01dd430f9bf473acf637b59fb761c4208e70c76f97dfeaffcc09949c515cd3e",
    "docs/GATEWAY_RELIABILITY_IMPLEMENTATION_PLAN.md": "793ada20a7c64ff990acdefd5718d62129fa897a5f8811cc39bc69d8071f4aab",
    "docs/gateway-reliability/G0_EVIDENCE.md": "d36162c11ff3629a51628c85578919917de7239b1b50e1a7f6c9470965717e81",
    "docs/gateway-reliability/IMPLEMENTATION_PROGRESS.md": "2c2a3eb8cb420f47257a909f61bd7f7a6b52648dfb06037dbcc9dc8658a24af0",
    "src/gateway/writes.rs": "67ec369eab9ae65a41a6d7ad1cc67ccadd6bb1796281e5a265d9bbd26a6be624"
  }
}
```
