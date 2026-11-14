# Claude Code review, iteration 6

Score: **8.5/10**. Historical snapshot with supplied native evidence.

Actual model: `claude-sonnet-5-5`; reviewer used read-only tools.
Native execution was performed by the implementer and reviewed as recorded evidence.
All scoped hashes were verified unchanged after review.

## Structured result

```json
{
  "repository": "prisma-airs-cli",
  "score": 8.5,
  "assessment": "I found no current source defect in the scoped change set. The two documented runtime fixes are present and consistent with the evidence. The prepared-request module, the sanitized preview, the shared preparation adapter and the debug lifecycle event all match their docs. The native evidence also fits the code: the manifest SHA-256 values for the 11 listed source files, Cargo.toml and Cargo.lock agree with evidence.json by inspection, and the six log hashes in the manifest match the recorded ones. I could not recompute any hash. The logs are 79/79, 86/86, 14/14, 2/2 and 2/2, with no `warning:` match in the Clippy log. Remaining limits are the ones the scope itself defers: unexecuted MSRV 1.89, other platforms and live service contracts, and CLI adoption of the SDK diagnostics getter. Those are deferred work, not defects. I'm holding the score below 9 because this is single-platform, single-toolchain, synthetic-fixture evidence, and the CLI still emits its generic schema messages until the SDK bridge lands.",
  "findings": [],
  "verification": {
    "executed": [
      "Read the scope file, manifest.json, the full tracked diff, NATIVE_CHECKS.md, evidence.json and IMPLEMENTATION_PROGRESS.md.",
      "Read src/gateway/request.rs and src/gateway/preview.rs in full, including their unit tests (6 preparation and 3 projection).",
      "Read all of tests/gateway_writes/reliability.rs (24 incident/reliability tests).",
      "Read the command driver diff in src/commands/gateway_writes.rs: prepare_request, preview, connect and authorise. Checked connect's new local-context, require_management, prepare_request, then GatewayClients::build order, and the conversion of the client-build error through `.into()` plus nothing_changed.",
      "Read the gateway.rs, gateway_write_plan.rs, cli.rs and Write::diagnostic_operation diffs.",
      "Read the debug.rs diff with its direct event and secret-suppression tests, and the generated gateway reference and user/design/compatibility doc diffs.",
      "Checked the pinned scm-sdk 0.2.4 source (generated.rs): GatewayServiceApiKeyCreateRequest and GatewayUserApiKeyCreateRequest both require `type` as a String, so the 'Required credential type' help is accurate for service and user keys. Both also have a Serialize impl, which credential_wire uses.",
      "Grepped the raw native logs for results and warnings: the gateway-write log passes 79 tests, the unit log 86, the debug log 14, and the generate and check reference logs 2 each. The Clippy log finishes, and no `warning:` match was found in any log.",
      "Compared the manifest hashes with evidence.json's tested_source_sha256 and log hashes by inspection. Source and Cargo hashes match, as do all six log hashes.",
      "Confirmed that the dry-run path creates no clients, requests no credentials, does not reserve the destination and does not prompt. This comes from reading the code and the tests, and the tests passed in the recorded run."
    ],
    "not_executed": [
      "I ran nothing myself: no builds, tests, hashing or services. The results above are from the implementer's recorded run.",
      "Not read in full: schema-evidence.json, generated-parser-evidence.md and the previous-review JSON.",
      "Not read in full: the long clippy and gateway-write log bodies beyond the result and warning lines, G0_EVIDENCE.md and the implementation plan beyond their status.",
      "Rust 1.89 MSRV, other platforms and profiles.",
      "Live service, fleet, package/release and SDK-to-CLI integration checks (not claimed)."
    ],
    "limitations": [
      "The hash binding is by inspection only. I could not independently confirm that the executed copies equal the current files.",
      "Local public-schema fixtures establish local behavior, not the service's key-type vocabulary or deployment replacement/merge semantics.",
      "The 8 MiB stack worker for the deep SDK fixture is not qualification of the default worker stack.",
      "Passing native logs do not substitute for the code review above, and this is not release acceptance."
    ]
  },
  "deferred_work": [
    "Adopt an immutable SDK diagnostics candidate through the CLI error bridge, then verify the field hints.",
    "Capture G0.3 service evidence for the organisation-response mismatch, key-type vocabulary and deployment replacement/merge semantics.",
    "Run MSRV 1.89 and other admitted platform/profile checks.",
    "Fleet qualification and any separately authorized release or publish work.",
    "Broaden preparation telemetry beyond typed-validation and tenant-binding refusals, if wanted."
  ],
  "verdict": "No findings against the scoped source. The native logs match the current source hashes by inspection, and the runtime fixes and docs are consistent with the code. The scope is acceptable as locally verified on Rust 1.99 macOS ARM64 with synthetic fixtures. It is not accepted for live-service, fleet, MSRV, package/release or SDK-integration purposes, which stay deferred."
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
    "docs/gateway-reliability/IMPLEMENTATION_PROGRESS.md": "03dff48002e16e4b178a83c5d69aba21ac2ccc5c2f2ea3417a59993f9e3593c9",
    "src/gateway/writes.rs": "67ec369eab9ae65a41a6d7ad1cc67ccadd6bb1796281e5a265d9bbd26a6be624",
    "docs-site/docs/reference/commands/gateway.md": "1835e4340245fd3330baa86c2a0f98c3a482dd3a7736a2c2cc37be9aa47b76f0",
    "docs/gateway-reliability/NATIVE_CHECKS.md": "26df8d4f8b159695f363eab211632599e3a6ca7b559f20c5212319b2a5ec4fc8",
    "docs/gateway-reliability/native-checks/evidence.json": "ad67e2a15717f1f36a75b5d446afbdda8784f12f2ceed5ba82297205fc54529e",
    "docs/gateway-reliability/native-checks/cli-reference-check.log": "955ada51b8f5a8522163676e34a597296a126168bc3c9a4f1977b2d646fbe2ff",
    "docs/gateway-reliability/native-checks/cli-gateway-unit.log": "aa54e2aab32bcc6be7569e2bf062462fca416376cdbbedde989c54fcb9b04777",
    "docs/gateway-reliability/native-checks/cli-debug-unit.log": "538333b4f4429d2865253709055cf05292eeb9bea8698ce7fea42c2722d8453f",
    "docs/gateway-reliability/native-checks/cli-reference-generate.log": "9073b67897303981b3e503983f15fb0fd6f8203278bd7a59101a04df370361ad",
    "docs/gateway-reliability/native-checks/cli-clippy.log": "6d23fe6d09c151d2d0e1f093378266ab16d13e15deb0878405f92fb8333f459e",
    "docs/gateway-reliability/native-checks/cli-gateway-writes.log": "33747476a3e79f30b99fb119fe29894d0ce4f587e32af5fc06e4eba318d0dd5f"
  }
}
```
