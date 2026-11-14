# Claude Code scoped review: prisma-airs-cli

Score: **6.5/10**. Source review only; no compilation or native tests were run.

Reviewer: Claude Code, model `claude-sonnet-5-5`. Independent session; read-only Read/Grep/Glob tools, no project commands, edits, external MCPs or subagents. Completed 2026-11-13T10:11:32.106552+00:00.

Reviewed base HEAD: `e235a4d0aa431eb0929c129f0ae0f6d008b07d7c` plus the scoped working-tree changes below. All scoped SHA-256 values were verified unchanged after review. This report is a new artifact and was not part of the reviewed implementation.

## Claude assessment

This is a source-review score only. I executed nothing: no Cargo build, no tests, no CLI run. The review is static reading of the scoped diff plus the surrounding writes.rs, plan, secrets and withhold code. The CLI stays on scm-sdk 0.2.4, and SDK diagnostics are not adopted here, which is a documented deferral. The design is sound. PreparedRequest is a pure seam that both execution and preview use. Preview builds its body from the typed wire serializer and shows only an allowlisted projection. The dry-run path does not call gateway_clients, does not authorize, does not prompt and does not reserve the secret destination. It does check the destination read-only, via check_destination in prepare(). Docs are mostly honest about what is deferred. I found no confirmed compile error and no confirmed redaction leak. I did find real gaps. The debug validation_failure event is wrongly attached to a tenant-binding refusal in preview and is missing from the execution-side re-validation paths. The withholding policy applied to the preview is effectively inert. Several tests are weak, and the tests that matter most are unrun. I held the score down for unexecuted code with real regression risk in the shared write path, for the incomplete diagnostic metadata, and for test quality.

## Findings

### 1. MEDIUM: validation_failure debug event is mislabeled for tenant-binding errors and missing on post-tenant validation failures

Location: `src/commands/gateway_writes.rs:210`.

preview() calls PreparedRequest::prepare(Context::Tenant{..}).inspect_err(|_| log.gateway_validation_rejected()) for every CliError. That call also returns the foreign_tsg refusals built in src/gateway/request.rs:42-57, which are tenant or organisation mismatches, not schema failures. The event is hard-coded to stage 'request_validation' (crates/airs-runtime-core/src/debug.rs:123-133). In execution, connect() (line 287) and authorise() (line 346) call the same prepare() with no logging. A body that passes preflight but fails binding therefore leaves the empty debug log this change set out to fix. A mismatch in preview is logged as a request-validation failure.

Impact: Debug-log consumers get a false failure stage for tenant mismatches in preview. They get no event at all for post-tenant refusals in real execution. This partly contradicts the claim in docs and COMPATIBILITY that semantic write-validation failures now record an event.

Recommendation: Make the event reflect the failure class: emit it only for schema or refinement failures, or add a stage or reason discriminator. Apply the logging consistently to connect() and authorise(), or document the limitation.

### 2. MEDIUM: Withholding policy applied to the preview is effectively a no-op because the document is nested

Location: `src/gateway/preview.rs:175`.

withhold::apply(&mut view, write.policy(), secrets) runs on a wrapper document that puts the body under request.body. The API_KEYS and DEPLOYMENTS policies use resource-relative rule paths (crates/airs-runtime-core/src/withhold.rs:225-231). Those paths will not match inside that wrapper, so only the SecretSet scrubbing is likely to take effect. I did not trace the Policy rule definitions, so this is a medium-confidence inference. The real protection is the hand-written allowlist projection (preview.rs:86-144). That projection does not use the shared withholding primitives, although the plan and DESIGN text say it is 'built on existing withholding primitives'.

Impact: There is no confirmed leak today, because the allowlist is conservative. But a future FIELDS or projection change would lack the defence in depth that the docs imply. The rules are now duplicated, with a hand-maintained list of types, statuses and scopes that can drift from the SDK vocabulary.

Recommendation: Apply the policy to the projected body at the correct root, or drop the call and fix the documentation. Add a test that exercises the policy path.

### 3. LOW: Preview projection passes several string fields through verbatim

Location: `src/gateway/preview.rs:95`.

organisation_id, workspace_id and expires_at are cloned verbatim, and the booleans in auth_settings are cloned as typed. Their values are only constrained by the SDK schema, and I did not verify whether that schema enforces a format on organisation_id or expires_at. An operator can pass any schema-valid string, including a secret-looking one, through --set organisation_id=... and see it in the preview. Only registered secrets are scrubbed. The unit test covers only name, type, defaults and gateway_base_url.

Impact: Low. These are ordinarily identifiers or timestamps, and the output goes to the operator's own terminal. It is inconsistent with the 'arbitrary content is withheld' claim.

Recommendation: Validate the shape of organisation_id, workspace_id and expires_at before echoing them, or withhold on mismatch. Add tests.

### 4. LOW: Weak and incomplete preview and dry-run tests

Location: `tests/gateway_writes/reliability.rs:386`.

The second half of dry_run_does_not_bypass_schema_or_tenant_validation asserts only that the exit code is non-zero, that stdout is empty and that there were no requests. It would pass for any unrelated failure, such as a usage error, and does not verify a foreign-tenant refusal. key_dry_run_checks_but_never_reserves_a_supplied_destination expects exit 1 and passes if any operational failure occurs. No test covers dry-run with --debug, text or quiet output, a deployment preview with rotate_auth, or CLI-level preview redaction of gateway_base_url. The 'redacted' flag is never asserted. The key-type tests also prove only that the local schema accepts the value, not that the service does; the file header says so.

Impact: These regressions could slip through once the suite is run.

Recommendation: Assert the specific error text or code, and add the missing cases.

### 5. LOW: Structured error metadata is not yet carried into debug events

Location: `crates/airs-runtime-core/src/debug.rs:123`.

The event contains only fixed strings: operation 'gateway.write', stage 'request_validation' and requestSent=false. There is no operation or route identifier, no schema identifier and no field issue. The progress document states this. docs/DESIGN.md says D38/D116 generic messages remain until the SDK is adopted.

Impact: The incident goal of discovering why a request was rejected is not met by the CLI yet. This is deferred G3.1/G3.2 work, not a defect in the current behavior. Hard-coding the operation to 'gateway.write' is the one place it is less informative than it could be even with the pinned 0.2.4.

Recommendation: Include the Write kind or route in the event now. Adopt the SDK diagnostics later as planned.

## Verification and limitations

Reviewed:

- Static reading of the scope manifest, the tracked diffs, the full new files, and surrounding code in gateway writes.rs, write plan, secrets, views, withhold and json.


Not executed:

- Cargo build, type-check, clippy and all tests, including tests/gateway_writes/reliability.rs and the unit tests in request.rs and preview.rs

- The CLI binary, live services and tenant or credential stores


Limitations:

- No compile evidence. I did not independently verify borrow-check details of the closures, such as the inspect_err closure borrowing invocation.debug, or the partial moves of Prepared fields in preview().

- I did not verify that load_tenant avoids requiring management credentials, which the dry-run test and docs claim.

- I did not trace withhold::Policy rule paths in full, so the finding that the policy is inert is an inference.

- I did not read the SDK-side changes; the SDK is reviewed separately and the CLI stays on scm-sdk 0.2.4.

- Test pass or fail status is unknown. The only checks the progress record claims are rustfmt and git diff --check, which are not type-checks.


## Deferred work

- G3.1: adopt the SDK diagnostic getter and carry it through runtime-core ErrorFacts, so CLI stderr shows field-level hints

- Add SDK field and stage detail to the lifecycle debug event

- G3.3/G3.4: service-contract evidence for deployment and key vocabularies, and the organisation response fix

- Regenerate the CLI reference docs; one exact SDK pin across all four consumers

- Native execution, MSRV and feature checks, and the broader G5 workflow qualification


## Claude verdict

A sound architecture with no confirmed compile error or leak, but unexecuted. The debug-event stage is wrong for tenant mismatches and missing on the execution paths, the preview withholding policy is likely inert, and several tests are weak. Source-review score 6.5/10; native execution is required before this can be accepted.

## Scope binding

| Path | SHA-256 |
| --- | --- |
| `crates/airs-runtime-core/src/debug.rs` | `f04f4d95ea062e3feb6be040106be6574145f035e7c057dc0eb4bafeaeded976` |
| `src/cli.rs` | `4431de23f4e185caac493bf807ccf10dd1feda47b4ff62e5eaea226d09522f78` |
| `src/commands/gateway.rs` | `6c2274c628dca727a4575f7ae00083126423aeaa13d13881da0c6ab8352eda2d` |
| `src/commands/gateway_write_plan.rs` | `47f4c77e94655f83514730545f854c5ab57e602413e45e5a547de5c7abfb8513` |
| `src/commands/gateway_writes.rs` | `3f77e2874034c038a739de396a4908d917319cc2261a088e3e7cad73982a382f` |
| `src/gateway/mod.rs` | `14bc95a1175514468d0daaa641067e0a6ac32dd131c39f282da82db8a674284e` |
| `src/gateway/request.rs` | `d4092e8cdd9b861de37cbfcbed8016fdabf63155517393116ff3dd06683429c1` |
| `src/gateway/preview.rs` | `7ef574179315d53058906bedba53d0b3667858bf3377099f1157b7fa992b484b` |
| `tests/gateway_writes.rs` | `898ea1501c6975db8e22d74bfbf7132a657a4176362f597d3c3f22cfd90f20ee` |
| `tests/gateway_writes/reliability.rs` | `e5b50dd4f14d696c888772664c5fd23c60a666394539b402d18eff456a0986b9` |
| `docs-site/docs/commands/gateway.md` | `60d6f75c6b4ec9e63d8544b5b0678dc50907c6d8158ef1af75ba525051ffc88a` |
| `docs/COMPATIBILITY.md` | `13d415e1f870c9e580dd833ae2d68083d563772cf06f012ea7e1397bbe44397e` |
| `docs/DESIGN.md` | `83ad76692436006d67ea4eac6a97e56902a827773f8151144d70681213aecc6a` |
| `docs/GATEWAY_RELIABILITY_IMPLEMENTATION_PLAN.md` | `793ada20a7c64ff990acdefd5718d62129fa897a5f8811cc39bc69d8071f4aab` |
| `docs/gateway-reliability/G0_EVIDENCE.md` | `d36162c11ff3629a51628c85578919917de7239b1b50e1a7f6c9470965717e81` |
| `docs/gateway-reliability/IMPLEMENTATION_PROGRESS.md` | `a0e8ed8e88c258c6cf8535535c8d056e96b0945b05a9de41bbf4aa5ba752fd22` |

## Original structured assessment

```json

{
  "repository": "prisma-airs-cli",
  "score": 6.5,
  "assessment": "This is a source-review score only. I executed nothing: no Cargo build, no tests, no CLI run. The review is static reading of the scoped diff plus the surrounding writes.rs, plan, secrets and withhold code. The CLI stays on scm-sdk 0.2.4, and SDK diagnostics are not adopted here, which is a documented deferral. The design is sound. PreparedRequest is a pure seam that both execution and preview use. Preview builds its body from the typed wire serializer and shows only an allowlisted projection. The dry-run path does not call gateway_clients, does not authorize, does not prompt and does not reserve the secret destination. It does check the destination read-only, via check_destination in prepare(). Docs are mostly honest about what is deferred. I found no confirmed compile error and no confirmed redaction leak. I did find real gaps. The debug validation_failure event is wrongly attached to a tenant-binding refusal in preview and is missing from the execution-side re-validation paths. The withholding policy applied to the preview is effectively inert. Several tests are weak, and the tests that matter most are unrun. I held the score down for unexecuted code with real regression risk in the shared write path, for the incomplete diagnostic metadata, and for test quality.",
  "findings": [
    {
      "severity": "medium",
      "title": "validation_failure debug event is mislabeled for tenant-binding errors and missing on post-tenant validation failures",
      "path": "src/commands/gateway_writes.rs",
      "line": 210,
      "evidence": "preview() calls PreparedRequest::prepare(Context::Tenant{..}).inspect_err(|_| log.gateway_validation_rejected()) for every CliError. That call also returns the foreign_tsg refusals built in src/gateway/request.rs:42-57, which are tenant or organisation mismatches, not schema failures. The event is hard-coded to stage 'request_validation' (crates/airs-runtime-core/src/debug.rs:123-133). In execution, connect() (line 287) and authorise() (line 346) call the same prepare() with no logging. A body that passes preflight but fails binding therefore leaves the empty debug log this change set out to fix. A mismatch in preview is logged as a request-validation failure.",
      "impact": "Debug-log consumers get a false failure stage for tenant mismatches in preview. They get no event at all for post-tenant refusals in real execution. This partly contradicts the claim in docs and COMPATIBILITY that semantic write-validation failures now record an event.",
      "recommendation": "Make the event reflect the failure class: emit it only for schema or refinement failures, or add a stage or reason discriminator. Apply the logging consistently to connect() and authorise(), or document the limitation."
    },
    {
      "severity": "medium",
      "title": "Withholding policy applied to the preview is effectively a no-op because the document is nested",
      "path": "src/gateway/preview.rs",
      "line": 175,
      "evidence": "withhold::apply(&mut view, write.policy(), secrets) runs on a wrapper document that puts the body under request.body. The API_KEYS and DEPLOYMENTS policies use resource-relative rule paths (crates/airs-runtime-core/src/withhold.rs:225-231). Those paths will not match inside that wrapper, so only the SecretSet scrubbing is likely to take effect. I did not trace the Policy rule definitions, so this is a medium-confidence inference. The real protection is the hand-written allowlist projection (preview.rs:86-144). That projection does not use the shared withholding primitives, although the plan and DESIGN text say it is 'built on existing withholding primitives'.",
      "impact": "There is no confirmed leak today, because the allowlist is conservative. But a future FIELDS or projection change would lack the defence in depth that the docs imply. The rules are now duplicated, with a hand-maintained list of types, statuses and scopes that can drift from the SDK vocabulary.",
      "recommendation": "Apply the policy to the projected body at the correct root, or drop the call and fix the documentation. Add a test that exercises the policy path."
    },
    {
      "severity": "low",
      "title": "Preview projection passes several string fields through verbatim",
      "path": "src/gateway/preview.rs",
      "line": 95,
      "evidence": "organisation_id, workspace_id and expires_at are cloned verbatim, and the booleans in auth_settings are cloned as typed. Their values are only constrained by the SDK schema, and I did not verify whether that schema enforces a format on organisation_id or expires_at. An operator can pass any schema-valid string, including a secret-looking one, through --set organisation_id=... and see it in the preview. Only registered secrets are scrubbed. The unit test covers only name, type, defaults and gateway_base_url.",
      "impact": "Low. These are ordinarily identifiers or timestamps, and the output goes to the operator's own terminal. It is inconsistent with the 'arbitrary content is withheld' claim.",
      "recommendation": "Validate the shape of organisation_id, workspace_id and expires_at before echoing them, or withhold on mismatch. Add tests."
    },
    {
      "severity": "low",
      "title": "Weak and incomplete preview and dry-run tests",
      "path": "tests/gateway_writes/reliability.rs",
      "line": 386,
      "evidence": "The second half of dry_run_does_not_bypass_schema_or_tenant_validation asserts only that the exit code is non-zero, that stdout is empty and that there were no requests. It would pass for any unrelated failure, such as a usage error, and does not verify a foreign-tenant refusal. key_dry_run_checks_but_never_reserves_a_supplied_destination expects exit 1 and passes if any operational failure occurs. No test covers dry-run with --debug, text or quiet output, a deployment preview with rotate_auth, or CLI-level preview redaction of gateway_base_url. The 'redacted' flag is never asserted. The key-type tests also prove only that the local schema accepts the value, not that the service does; the file header says so.",
      "impact": "These regressions could slip through once the suite is run.",
      "recommendation": "Assert the specific error text or code, and add the missing cases."
    },
    {
      "severity": "low",
      "title": "Structured error metadata is not yet carried into debug events",
      "path": "crates/airs-runtime-core/src/debug.rs",
      "line": 123,
      "evidence": "The event contains only fixed strings: operation 'gateway.write', stage 'request_validation' and requestSent=false. There is no operation or route identifier, no schema identifier and no field issue. The progress document states this. docs/DESIGN.md says D38/D116 generic messages remain until the SDK is adopted.",
      "impact": "The incident goal of discovering why a request was rejected is not met by the CLI yet. This is deferred G3.1/G3.2 work, not a defect in the current behavior. Hard-coding the operation to 'gateway.write' is the one place it is less informative than it could be even with the pinned 0.2.4.",
      "recommendation": "Include the Write kind or route in the event now. Adopt the SDK diagnostics later as planned."
    }
  ],
  "verification": {
    "executed": [
      "Static reading of the scope manifest, the tracked diffs, the full new files, and surrounding code in gateway writes.rs, write plan, secrets, views, withhold and json."
    ],
    "not_executed": [
      "Cargo build, type-check, clippy and all tests, including tests/gateway_writes/reliability.rs and the unit tests in request.rs and preview.rs",
      "The CLI binary, live services and tenant or credential stores"
    ],
    "limitations": [
      "No compile evidence. I did not independently verify borrow-check details of the closures, such as the inspect_err closure borrowing invocation.debug, or the partial moves of Prepared fields in preview().",
      "I did not verify that load_tenant avoids requiring management credentials, which the dry-run test and docs claim.",
      "I did not trace withhold::Policy rule paths in full, so the finding that the policy is inert is an inference.",
      "I did not read the SDK-side changes; the SDK is reviewed separately and the CLI stays on scm-sdk 0.2.4.",
      "Test pass or fail status is unknown. The only checks the progress record claims are rustfmt and git diff --check, which are not type-checks."
    ]
  },
  "deferred_work": [
    "G3.1: adopt the SDK diagnostic getter and carry it through runtime-core ErrorFacts, so CLI stderr shows field-level hints",
    "Add SDK field and stage detail to the lifecycle debug event",
    "G3.3/G3.4: service-contract evidence for deployment and key vocabularies, and the organisation response fix",
    "Regenerate the CLI reference docs; one exact SDK pin across all four consumers",
    "Native execution, MSRV and feature checks, and the broader G5 workflow qualification"
  ],
  "verdict": "A sound architecture with no confirmed compile error or leak, but unexecuted. The debug-event stage is wrong for tenant mismatches and missing on the execution paths, the preview withholding policy is likely inert, and several tests are weak. Source-review score 6.5/10; native execution is required before this can be accepted."
}

```
