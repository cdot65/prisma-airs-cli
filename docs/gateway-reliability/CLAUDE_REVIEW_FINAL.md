# Final Claude Code review: prisma-airs-cli

**Score: 9/10. No required code changes remain in the declared scope.**

Actual reviewer: Claude Code, model `claude-sonnet-5-5`.
This is iteration9; earlier reports retain their original scores and snapshots.
The reviewer personally invoked the restricted verifier: all current source,
retained binary and log hashes matched, and **368 synthetic test executions**
passed across Rust1.89 and1.99 on macOS ARM64. Unit/debug replays used the default
parallel harness. The reviewer did not rebuild source or rerun Clippy; those
provided build/lint records are in the native-check evidence.

[Independent replay receipt and raw logs](native-checks/claude-final-replay.json) record what Claude executed.
The reviewed source hashes were rechecked unchanged after the review completed.

This is a scoped code-quality result, not release, live-service, other-platform
or fleet acceptance. Immutable SDK adoption in the CLI and actual service-contract
qualification remain separate work. Optional notes are preserved below; none is
required before merging this scope. No package versions, locks, frozen contracts
or published artifacts were changed.

The local MSRV setup also auto-updated the shared rustup launcher from1.29.1 to
1.29.3, as disclosed in the native environment evidence. No CLI install was replaced.

## Exact reviewer result

```json
{
  "repository": "/Users/cdot/development/goshawks.io/prisma-airs-cli",
  "score": 9,
  "assessment": "I found no correctness, security or regression defect in the scoped change that must be fixed. Request preparation is one pure module, `PreparedRequest::prepare` with Preflight and Tenant contexts, and execution, preview and the provider-resolution re-validation all use it. Failures are classified at their origin by `FailureKind`, not by parsing messages. Dry-run skips client construction, authentication, the confirmation gate and credential-file reservation. It still checks a supplied `--secret-output` path without reserving it, and tests cover that. The preview is an allow-list projection: unknown keys and unrecognised strings are withheld, and the envelope also passes the registered-secret scrubber. The lifecycle logger tries full facts, then a minimal fallback record, then a generic marker. Each candidate gets the same whole-line secret check, and if all three collide it writes nothing. The unit tests cover operation collision, generic fallback and every-marker collision. The missing-TSG concern is resolved as stated: `preview_and_execution_preserve_the_registered_tsg_pin_when_config_loses_it` exercises both modes and expects the identity-pin refusal before preparation. The pure API has a separate empty-selected-tenant refusal, and `request.rs` tests it. `Write::diagnostic_operation` is an exhaustive match, so a new write variant cannot compile without a static operation name. The remaining items are low-severity, documented tradeoffs or optional polish. Native evidence is strong for macOS ARM64 on both compilers, but it is synthetic and local, and it does not cover the service contract. I did not build or run anything from source.",
  "findings": [
    {
      "severity": "low",
      "title": "Hard-coded preview scope and status literals can drift from the CLI help and the accepted vocabulary",
      "path": "src/gateway/preview.rs",
      "line": 157,
      "evidence": "The five displayable scopes and the status literals `active|archived` are inline `matches!` lists in `body_view`. The scope list is duplicated separately in the `--scopes` help text. `DISPLAY_TYPE_LITERALS` (line 13) is a named constant, but scopes and status are not.",
      "impact": "A future vocabulary change could update help or validation without updating the preview. The result would be withheld, privacy-safe output, never a leak. The `redacted` flag also stays accurate.",
      "recommendation": "Optional polish: put the display literals for type, status and scopes in one named policy table and unit-test it against the help text. The docs already label this a privacy policy, not service support.",
      "required_before_merge": false
    },
    {
      "severity": "low",
      "title": "Fallback and suppression tiers are verified only at unit level; `SecretSet::add` has no minimum length",
      "path": "crates/airs-runtime-core/src/debug.rs",
      "line": 157,
      "evidence": "`gateway_preparation_rejected` checks each candidate with `appears_in_ignoring_case`. `SecretSet::add` (secrets.rs:227) accepts any non-empty string. A very short registered secret, such as a single character, would match structural tokens and suppress the whole lifecycle record. Only the debug unit tests exercise the fallback tiers; the CLI integration tests exercise the full-facts path.",
      "impact": "Confidentiality wins, which is the documented behaviour. The cost is a missing diagnostic record in a rare edge case, and the docs state that suppression can happen.",
      "recommendation": "Optional: add one end-to-end test with a registered secret that equals the static operation string. This is an accepted, documented tradeoff and not a defect.",
      "required_before_merge": false
    },
    {
      "severity": "low",
      "title": "Missing-selected-TSG message in the pure API is not reachable through the CLI",
      "path": "src/gateway/request.rs",
      "line": 76,
      "evidence": "Both CLI modes hit the registered-identity pin first, and `require_management` runs before preparation in execution. The `Selected tenant is missing mgmtTsgId` branch is covered only by a unit test.",
      "impact": "This is defensive code for the pure API. It has no user-visible effect today.",
      "recommendation": "No change needed. This is a documented defensive branch.",
      "required_before_merge": false
    },
    {
      "severity": "low",
      "title": "Preview type literals are a guess at the service vocabulary, so a legitimate type may be withheld",
      "path": "src/gateway/preview.rs",
      "line": 13,
      "evidence": "The display set is {production, non_production, workspace-service, service}. The documentation says it is a privacy policy and not a service-support list. Real service vocabulary is listed as separate work.",
      "impact": "An unlisted but valid type is shown as `[withheld]` and `redacted: true`. This is fail-closed, and the docs state it.",
      "recommendation": "No code change in this scope. Revisit when the real service vocabulary is established.",
      "required_before_merge": false
    }
  ],
  "verification": {
    "executed": [
      "I read the scope file, manifest, verify_native.py, the verify-cli-native wrapper and the head of verification-bundle.json.",
      "I read the full tracked diff and read request.rs, preview.rs and tests/gateway_writes/reliability.rs (lines 525-793) directly from the repo.",
      "I read the SecretSet add and appears_in_ignoring_case implementations and the destination() planner.",
      "I personally ran /var/folders/gb/80rbbhr936z3k_4n4m3fr0j00000gn/T/airs-gateway-claude-review-9-824_slj3/verify-cli-native with a 300000 ms timeout. It exited successfully, reporting hashes_verified true, all_passed true and tests_passed 368. The retained test binaries ran under a cleared environment: gateway_writes 80 on Rust 1.99 and 1.89, lib gateway:: 87 on both, debug 15 on both, cli_reference 2 on both.",
      "The verifier wrote its own logs and prisma-airs-cli-independent-verification.json. I did not open that JSON; I relied on the verifier's stdout."
    ],
    "not_executed": [
      "No Cargo build, clippy or fresh compile. The clippy result is only the supplied native log (cli-clippy9.log).",
      "No live service, real tenant or credential access.",
      "I did not independently read the SDK-side sources, the schema-evidence.json or the generated-parser-evidence.md. My review is limited to the CLI source and tests I read directly.",
      "I did not read the older native-check logs or most of the doc files line by line. I read the doc changes shown in the diff."
    ],
    "limitations": [
      "The verifier replays retained binaries whose bytes it hashes. Whether those binaries were built from the current source rests on the owner's hash bindings in the bundle. I did not rebuild them.",
      "Coverage is macOS ARM64 only, with synthetic and loopback fixtures. Real service acceptance, replace/merge behaviour, other platforms and the fleet are not covered.",
      "The unit and gateway selections ran under the default parallel harness, as designed."
    ]
  },
  "deferred_work": [
    "Real service vocabulary and replace/merge semantics for deployment auth settings and key types.",
    "SDK diagnostics adoption through the shared error bridge (D38/D116), plus SDK getter adoption.",
    "Other platforms and fleet acceptance.",
    "Package and publishing gates; no version change is implied.",
    "Optional: consolidate the preview display-literal policy and add an end-to-end fallback test."
  ],
  "required_changes_for_9": [],
  "verdict": "No required code changes remain in the declared CLI scope. Candid scoped score: 9, with only low-severity optional polish and documented tradeoffs outstanding. This is not release approval. Service, platform, SDK-adoption and package acceptance remain separate and unevaluated."
}
```

## Reviewed snapshot bindings

```json
{
  "repository": "/Users/cdot/development/goshawks.io/prisma-airs-cli",
  "head": "e235a4d0aa431eb0929c129f0ae0f6d008b07d7c",
  "files": {
    "crates/airs-runtime-core/src/debug.rs": "2c17b4aa1486a62efd4842c6c6595c9fcc6b1a1baf8fddd808ea5762f12a7ac5",
    "src/cli.rs": "4431de23f4e185caac493bf807ccf10dd1feda47b4ff62e5eaea226d09522f78",
    "src/commands/gateway.rs": "6c2274c628dca727a4575f7ae00083126423aeaa13d13881da0c6ab8352eda2d",
    "src/commands/gateway_write_plan.rs": "47f4c77e94655f83514730545f854c5ab57e602413e45e5a547de5c7abfb8513",
    "src/commands/gateway_writes.rs": "37de3c60aca39973d7d951b92c699304edcfc109d17594cd3c894ab3529ea936",
    "src/gateway/mod.rs": "14bc95a1175514468d0daaa641067e0a6ac32dd131c39f282da82db8a674284e",
    "src/gateway/request.rs": "06483d9a163ecc098d3d77e082b9342ee59878d145c2ff991440f917d1dccd97",
    "src/gateway/preview.rs": "641279de1bbbaa8a7291cc9597662023cd6b5b9d838b3535ab019a02a95e7f46",
    "tests/gateway_writes.rs": "898ea1501c6975db8e22d74bfbf7132a657a4176362f597d3c3f22cfd90f20ee",
    "tests/gateway_writes/reliability.rs": "3520c215ee375401e551a859ced5dfa3879687f8f784938c3438af5c5ea573a2",
    "docs-site/docs/commands/gateway.md": "5ec386c25dd351a5cebc99fa98c63a9b4dd36d5dc58ed28a988c5a397966c743",
    "docs/COMPATIBILITY.md": "13d415e1f870c9e580dd833ae2d68083d563772cf06f012ea7e1397bbe44397e",
    "docs/DESIGN.md": "146124774a5c75bab20cd25d9b6a721f42474f1bddaf952f685097c0955e7b24",
    "docs/GATEWAY_RELIABILITY_IMPLEMENTATION_PLAN.md": "27c97386d8da6960d297aae65f277b87d0b0ff9cdb5893f6d7a24dc97d49521d",
    "docs/gateway-reliability/G0_EVIDENCE.md": "d36162c11ff3629a51628c85578919917de7239b1b50e1a7f6c9470965717e81",
    "docs/gateway-reliability/IMPLEMENTATION_PROGRESS.md": "643939774a6e0123b7f1843924fee5f91b769159fc9a2e64fb5689a39c9669dd",
    "src/gateway/writes.rs": "67ec369eab9ae65a41a6d7ad1cc67ccadd6bb1796281e5a265d9bbd26a6be624",
    "docs-site/docs/reference/commands/gateway.md": "1835e4340245fd3330baa86c2a0f98c3a482dd3a7736a2c2cc37be9aa47b76f0",
    "docs/gateway-reliability/NATIVE_CHECKS.md": "aeffb1e19c851d68562ed1b38cfba294dd2063b272016a7e59f0131b7202718d",
    "docs/gateway-reliability/native-checks/evidence.json": "3db1d11ab1355c4b3fe1e61f65a8fe2abe1837787dfc5303d5ec8f87d91f01a8",
    "docs/gateway-reliability/native-checks/cli-reference-check.log": "955ada51b8f5a8522163676e34a597296a126168bc3c9a4f1977b2d646fbe2ff",
    "docs/gateway-reliability/native-checks/cli-gateway-unit.log": "aa54e2aab32bcc6be7569e2bf062462fca416376cdbbedde989c54fcb9b04777",
    "docs/gateway-reliability/native-checks/cli-debug-unit.log": "538333b4f4429d2865253709055cf05292eeb9bea8698ce7fea42c2722d8453f",
    "docs/gateway-reliability/native-checks/cli-reference-generate.log": "9073b67897303981b3e503983f15fb0fd6f8203278bd7a59101a04df370361ad",
    "docs/gateway-reliability/native-checks/cli-clippy.log": "6d23fe6d09c151d2d0e1f093378266ab16d13e15deb0878405f92fb8333f459e",
    "docs/gateway-reliability/native-checks/cli-gateway-writes.log": "33747476a3e79f30b99fb119fe29894d0ce4f587e32af5fc06e4eba318d0dd5f",
    "docs/gateway-reliability/native-checks/cli-msrv-debug.log": "3eb49a02bc62a00ef608ae1be2507e8096aaa4b2c61349ac3fa4d872fa239cf0",
    "docs/gateway-reliability/native-checks/cli-msrv-unit.log": "3a422cb247711a09089b45623b6f3d546fe81795f792f709944addd29e1f11bf",
    "docs/gateway-reliability/native-checks/cli-msrv-gateway.log": "f9f9659d53bb78acad4081f9cb76d52c1680984808d6f6f0711acec1fb93645a",
    "docs/gateway-reliability/native-checks/vulture-preview-receipt.json": "c9dfba89599d3ab02fb750116d5a1dbc58a324cf1cbfc76e36c4b30504d16127",
    "docs/gateway-reliability/native-checks/cli-msrv-reference.log": "e3a2e8e38104d7f65209fa017f96db4bfcf94b945c9f2a1f2b05df004c8c8fd7",
    "docs/gateway-reliability/native-checks/cli-gateway-qualified8.log": "25a67132ad864da29b259b10bffc694a0d66609f586ee489a856dbb8becf33e3",
    "docs/gateway-reliability/native-checks/cli-clippy8.log": "0e3a7d19624aa5b6c2d3dcb52a918636584d2498f295a050d54c475a747c68aa",
    "docs/gateway-reliability/native-checks/cli-unit-review8.log": "5d9684a725ec41fa2c919e4babf63e6956725070447b07acf142fcf1cf099cdb",
    "docs/gateway-reliability/native-checks/cli-msrv-unit8.log": "278ef1102ea2c5f4154b2267562c00507cc7244bb1c2d7ebc5ba91d99860e41a",
    "docs/gateway-reliability/native-checks/cli-reference8.log": "330f99f91fc710d201b970df41a5a193ce1ed7846cffd7b77eb7efc2ee28aa9d",
    "docs/gateway-reliability/native-checks/cli-msrv-gateway8.log": "33cd6af30193d32be2a5f5eb0a934dd236768074518886ace9a9f85bf1c30dae",
    "docs/gateway-reliability/native-checks/cli-msrv-reference8.log": "bb15c39e27d464f139460b8a29bb956bfbbb795410e3529254baa7409eb1c1a4",
    "docs/gateway-reliability/native-checks/cli-msrv-debug9.log": "a7a2feac5328ea0999753352ec4df7b5de5b3f1db8ebdc8e19e932eae0f638f3",
    "docs/gateway-reliability/native-checks/cli-clippy9.log": "13bba03bc735c96f25908050848bef2973c98077f78647157fd3822f9e63bf8c",
    "docs/gateway-reliability/native-checks/cli-msrv-unit9.log": "48e9e12b86a10520206b628670499275b3733b70387b834b9e22c2e6b67067e3",
    "docs/gateway-reliability/native-checks/cli-gateway9.log": "fe7bca6955212514e21b87e17d2e626864eb9bd69ffe9ffd89fa5096671fce3b",
    "docs/gateway-reliability/native-checks/cli-reference9.log": "ab1a67e09ff557c8ac7f475a8a0c532de6b0219f7f1b1383d9bf52f17c88de12",
    "docs/gateway-reliability/native-checks/cli-msrv-gateway9.log": "0fa62ec6c68e583593b70fdb3e7f28ce2058ee568a52c9b6ef2dfaaa855f60ce",
    "docs/gateway-reliability/native-checks/cli-msrv-reference9.log": "b159bb95bff1684a89d2b2ce977a3f95ad37e63c85957e5d229eafc3de843f0c",
    "docs/gateway-reliability/native-checks/cli-debug9.log": "7ce516044771255230aff64d643f3799b7eae73f42a8453160be76fa17b2235a",
    "docs/gateway-reliability/native-checks/cli-unit9.log": "1260d961d3b387e5d0271dd00b5a659e3a269dff4ff02315cdb1b4e69a48ed4a"
  }
}
```
