# Claude Code review, iteration 8

Score: **8.5/10**. Historical snapshot; later documentation/runtime changes require another review.

Model: `claude-sonnet-5-5`. Reviewer independently invoked the restricted
hash verifier and retained synthetic test binaries; all replay checks passed.

## Structured result

```json
{
  "repository": "prisma-airs-cli",
  "score": 8.5,
  "assessment": "No blocking source defects found in the scoped changes. Execution and preview share one pure preparation module (`PreparedRequest::prepare`). The old inline typed()/foreign_body_org logic in the driver is removed. In `connect`, `require_management` still runs before preparation and `GatewayClients::build` runs after it, so the ordering of the existing refusals is preserved. Preview builds no client and authenticates nothing. It also reserves no credential file and sends no request. The `--force` conflict is enforced by clap. The preview is built from allow-list projection. Unknown keys, arbitrary names, labels, dynamic keys, URLs and non-literal type/scope values are withheld. The registered-secret scrubber then runs over the whole envelope. The lifecycle debug record uses only static identifiers, is skipped (not written in redacted form) if it matches a registered secret, and has tests for both behaviours. Failures are classified at their origin by FailureKind, not by parsing messages. The missing-TSG concern is handled in two places. In the registered store, `load_raw` (src/tenant/mod.rs:204) pins mgmtTsgId, and the integration test at reliability.rs:761 confirms the identity refusal in both execution and preview. In the pure API, an empty selected TSG gives a clear message, with a unit test. Test coverage is broad: schema refusal before OAuth, three input sources for each operation, flag-versus-body tenant refusals logged exactly once, provider-resolution checkpoint logged after reads, the invalid-TSG format refusal, destination checked but not reserved, and the no-management-credentials preview. The replays I ran pass on both toolchains. The remaining items are low-severity polish and documented tradeoffs, not defects.",
  "findings": [
    {
      "severity": "low",
      "title": "Preview type allow-list is hard-coded and diverges from the unvalidated service vocabulary",
      "path": "src/gateway/preview.rs",
      "line": 130,
      "evidence": "body_view shows `type` only when it equals one of production, non_production, workspace-service or service, and withholds anything else. The guide says the service type vocabulary is unverified, and the pinned schema accepts any nonempty string.",
      "impact": "A valid service type outside the list previews as [withheld] and sets redacted: true. This is a privacy-safe failure and matches the documented trade-off, but the literal list can drift from the real vocabulary and users cannot tell what was withheld.",
      "recommendation": "Keep the list pinned to the documented vocabulary and revisit it when the service contract is confirmed. Alternatively, mark which fields were withheld."
    },
    {
      "severity": "low",
      "title": "Debug lifecycle record can be silently absent from a debug log",
      "path": "crates/airs-runtime-core/src/debug.rs",
      "line": 84,
      "evidence": "gateway_preparation_rejected writes nothing, with no warning, when a secret matches the static text. The test at debug.rs:1245 asserts the warning is empty.",
      "impact": "In the rare case that a registered secret is a substring of the static text (for example a very short secret), the debug log lacks the validation record, and the user gets no signal. The guide documents this, and it protects privacy.",
      "recommendation": "None required for correctness. Optionally emit a generic marker record that carries no operation name."
    },
    {
      "severity": "low",
      "title": "Preview-mode secret-destination checks differ from execution by design but can mislead",
      "path": "src/commands/gateway_write_plan.rs",
      "line": 479,
      "evidence": "`destination(&args.secret, !dry_run)` means a service-key preview does not require --secret-output, and a rotating deployment preview skips the NEEDS_DESTINATION usage error (gateway_writes.rs:638). The preview reports `secret_output_required` and `secret_output_supplied`.",
      "impact": "A preview exits 0 for a command that execution would refuse with a usage error. The guide and the requirements block document this, so it is an intended trade-off, but scripts that read only the exit code could be misled.",
      "recommendation": "Leave as is. Make sure consumers read execution_requirements."
    }
  ],
  "verification": {
    "executed": [
      "Read the scope file, verify_native.py, verify-cli-native and verification-bundle.json.",
      "Read the full tracked diff. Read request.rs and preview.rs (including their unit tests) in full, and all 793 lines of tests/gateway_writes/reliability.rs.",
      "Read gateway.rs gateway_clients and gateway_local_context. Read the pin logic in src/tenant/mod.rs (load_raw).",
      "I personally ran the single authorized command, verify-cli-native, with timeout 300000. It exited cleanly: hashes_verified true, all_passed true, 366 tests in total.",
      "Replayed counts: 80 gateway_writes integration tests on 1.99 and on 1.89 (MSRV). 87 gateway unit tests on 1.99 and on 1.89. 14 debug tests on 1.99 and on 1.89. 2 reference tests on 1.99 and on 1.89.",
      "Read the resulting prisma-airs-cli-independent-verification.json. I saw only the first 20 lines, which confirm hashes_verified and the first check records. The full results came from the command's stdout."
    ],
    "not_executed": [
      "No Cargo build, clippy or other command ran. The clippy result (cli-clippy8.log) was not replayed.",
      "I did not open the new docs/gateway-reliability files, G0_EVIDENCE.md, IMPLEMENTATION_PROGRESS.md, NATIVE_CHECKS.md or evidence.json individually. I did not read schema-evidence.json, generated-parser-evidence.md or the previous-review JSON. I did not read writes.rs beyond the diff, or the full cli.rs and plan code beyond the diff hunks.",
      "I did not read the vulture preview receipt.",
      "AI-Red-Teaming-bulk-scan.csv and brief/ were excluded, as instructed.",
      "No live service, other-platform or fleet checks."
    ],
    "limitations": [
      "Process deviation, disclosed: I also ran `wc -l` on reliability.rs through Bash. That was outside the single authorized command. It was read-only and had no effect on source or evidence, but it did not comply with the restriction.",
      "Replays use the frozen, hash-bound binaries on macOS ARM64 with synthetic and loopback fixtures only. They do not exercise current source freshly, because I did not rebuild.",
      "The integration tests use a fixture server, not the real service. Local preparation and fixture traffic are covered, but service acceptance is not.",
      "The service's type vocabulary and the deployment replacement/merge semantics are unverified. The guide documents this, and the tests state it.",
      "The integration and gateway-unit suites were replayed with --test-threads=1 or with the default harness as configured in the bundle.",
      "Because I did not read every documentation file, my doc-accuracy assessment covers the diffed docs (gateway.md, COMPATIBILITY.md and DESIGN.md amendments) only."
    ]
  },
  "deferred_work": [
    "Adoption of the SDK diagnostics getters through the shared error bridge. The CLI stays on scm-sdk 0.2.4, and the generic schema messages remain until adoption (per DESIGN).",
    "Confirming the real service's key type vocabulary and the deployment auth-settings replacement/merge semantics.",
    "Live-service, other-platform (non-macOS-ARM64) and fleet acceptance.",
    "Release, version and publishing work. None is authorized or implied.",
    "Complete preparation telemetry for file, credential, confirmation and config-load refusals. The design says these remain stderr-only."
  ],
  "verdict": "Scoped code review passes with no blocking defects. The shared preparation path, the preview privacy projection, the debug lifecycle adapter and the tenant-pin behaviour are sound and well tested. The only items are low-severity documented trade-offs. This is a scoped review, not release approval. Service, platform, SDK-adoption and release acceptance remain separate."
}
```

## Independent replay receipt

```json
{
  "repository": "prisma-airs-cli",
  "hashes_verified": true,
  "checks": [
    {
      "name": "cli-gateway-qualified8",
      "exit_code": 0,
      "passed": 80,
      "expected": 80,
      "elapsed_seconds": 26.879,
      "log": "/private/var/folders/gb/80rbbhr936z3k_4n4m3fr0j00000gn/T/airs-gateway-claude-review-8-fw41sr01/prisma-airs-cli-independent-cli-gateway-qualified8.log",
      "log_sha256": "c8c5ebc5e7d072d84ef0bb093f0f668954b5e033a09d1ebc70cea434b4ad2025"
    },
    {
      "name": "cli-msrv-gateway8",
      "exit_code": 0,
      "passed": 80,
      "expected": 80,
      "elapsed_seconds": 30.167,
      "log": "/private/var/folders/gb/80rbbhr936z3k_4n4m3fr0j00000gn/T/airs-gateway-claude-review-8-fw41sr01/prisma-airs-cli-independent-cli-msrv-gateway8.log",
      "log_sha256": "ad8c6acc39635dab59d8a40e3e2eb43b07d784df0df83e33fde03b3f7c169a32"
    },
    {
      "name": "cli-unit-review8",
      "exit_code": 0,
      "passed": 87,
      "expected": 87,
      "elapsed_seconds": 5.397,
      "log": "/private/var/folders/gb/80rbbhr936z3k_4n4m3fr0j00000gn/T/airs-gateway-claude-review-8-fw41sr01/prisma-airs-cli-independent-cli-unit-review8.log",
      "log_sha256": "d579055f43466a6e5107bc323c53bf910bebe4bd7d075c8adae133e707c42d1c"
    },
    {
      "name": "cli-msrv-unit8",
      "exit_code": 0,
      "passed": 87,
      "expected": 87,
      "elapsed_seconds": 5.844,
      "log": "/private/var/folders/gb/80rbbhr936z3k_4n4m3fr0j00000gn/T/airs-gateway-claude-review-8-fw41sr01/prisma-airs-cli-independent-cli-msrv-unit8.log",
      "log_sha256": "93ed620193682052e21bbde7e0c1703f222ff1e5cd293351354e20d66183c6b6"
    },
    {
      "name": "cli-debug-unit",
      "exit_code": 0,
      "passed": 14,
      "expected": 14,
      "elapsed_seconds": 0.614,
      "log": "/private/var/folders/gb/80rbbhr936z3k_4n4m3fr0j00000gn/T/airs-gateway-claude-review-8-fw41sr01/prisma-airs-cli-independent-cli-debug-unit.log",
      "log_sha256": "8e9e09b8c1ae6470a06e00af39691ed798a3242b3f1e8102c5b9dc1fa21174fd"
    },
    {
      "name": "cli-msrv-debug",
      "exit_code": 0,
      "passed": 14,
      "expected": 14,
      "elapsed_seconds": 0.542,
      "log": "/private/var/folders/gb/80rbbhr936z3k_4n4m3fr0j00000gn/T/airs-gateway-claude-review-8-fw41sr01/prisma-airs-cli-independent-cli-msrv-debug.log",
      "log_sha256": "7c8f7cee2a55f15ae52cd11c26a1549c174486697fc315bc9f1584101ca24bca"
    },
    {
      "name": "cli-reference8",
      "exit_code": 0,
      "passed": 2,
      "expected": 2,
      "elapsed_seconds": 6.273,
      "log": "/private/var/folders/gb/80rbbhr936z3k_4n4m3fr0j00000gn/T/airs-gateway-claude-review-8-fw41sr01/prisma-airs-cli-independent-cli-reference8.log",
      "log_sha256": "f8727320b8a3d46b11708e7d32e5a0caa48325baef47c0659cfbed64aec6cb6d"
    },
    {
      "name": "cli-msrv-reference8",
      "exit_code": 0,
      "passed": 2,
      "expected": 2,
      "elapsed_seconds": 6.781,
      "log": "/private/var/folders/gb/80rbbhr936z3k_4n4m3fr0j00000gn/T/airs-gateway-claude-review-8-fw41sr01/prisma-airs-cli-independent-cli-msrv-reference8.log",
      "log_sha256": "5b2c74aefa4f033e51848f635c356eff72afeb3a0f874db7061ec24ffd9c339e"
    }
  ],
  "all_passed": true,
  "note": "Replays only retained synthetic/loopback test binaries with a cleared environment; no Cargo, source edits, config stores or live service access."
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
    "src/gateway/request.rs": "06483d9a163ecc098d3d77e082b9342ee59878d145c2ff991440f917d1dccd97",
    "src/gateway/preview.rs": "068c2bd2ffb8849885e20b0f40459ccf828c66b116799d06faa132576ce455f4",
    "tests/gateway_writes.rs": "898ea1501c6975db8e22d74bfbf7132a657a4176362f597d3c3f22cfd90f20ee",
    "tests/gateway_writes/reliability.rs": "3520c215ee375401e551a859ced5dfa3879687f8f784938c3438af5c5ea573a2",
    "docs-site/docs/commands/gateway.md": "e252cfc575f250374bf8a2926309d3428546067f6aaf0b9f92c8792f3d0c661d",
    "docs/COMPATIBILITY.md": "13d415e1f870c9e580dd833ae2d68083d563772cf06f012ea7e1397bbe44397e",
    "docs/DESIGN.md": "4b2e63aa6a937dae1978147b7c61ca782f521ad40b69bb4743ab2ca168c2e1b7",
    "docs/GATEWAY_RELIABILITY_IMPLEMENTATION_PLAN.md": "27c97386d8da6960d297aae65f277b87d0b0ff9cdb5893f6d7a24dc97d49521d",
    "docs/gateway-reliability/G0_EVIDENCE.md": "d36162c11ff3629a51628c85578919917de7239b1b50e1a7f6c9470965717e81",
    "docs/gateway-reliability/IMPLEMENTATION_PROGRESS.md": "c7bdf4677bbc0e5a21476931bc0a56ae1b41b6e626732c41fbaeb8ef72b1c839",
    "src/gateway/writes.rs": "67ec369eab9ae65a41a6d7ad1cc67ccadd6bb1796281e5a265d9bbd26a6be624",
    "docs-site/docs/reference/commands/gateway.md": "1835e4340245fd3330baa86c2a0f98c3a482dd3a7736a2c2cc37be9aa47b76f0",
    "docs/gateway-reliability/NATIVE_CHECKS.md": "2c01de590171784f87f49e1b69f10e1901f9e978e3dc4001b3ad6860122186b3",
    "docs/gateway-reliability/native-checks/evidence.json": "48d0e1bb511c6a75fed928db119f80c1fcd3abf1b829ad1ed5e497a6b8ccbf0d",
    "docs/gateway-reliability/native-checks/cli-reference-check.log": "955ada51b8f5a8522163676e34a597296a126168bc3c9a4f1977b2d646fbe2ff",
    "docs/gateway-reliability/native-checks/cli-gateway-unit.log": "aa54e2aab32bcc6be7569e2bf062462fca416376cdbbedde989c54fcb9b04777",
    "docs/gateway-reliability/native-checks/cli-debug-unit.log": "538333b4f4429d2865253709055cf05292eeb9bea8698ce7fea42c2722d8453f",
    "docs/gateway-reliability/native-checks/cli-reference-generate.log": "9073b67897303981b3e503983f15fb0fd6f8203278bd7a59101a04df370361ad",
    "docs/gateway-reliability/native-checks/cli-clippy.log": "6d23fe6d09c151d2d0e1f093378266ab16d13e15deb0878405f92fb8333f459e",
    "docs/gateway-reliability/native-checks/cli-gateway-writes.log": "33747476a3e79f30b99fb119fe29894d0ce4f587e32af5fc06e4eba318d0dd5f",
    "docs/gateway-reliability/native-checks/cli-msrv-debug.log": "3eb49a02bc62a00ef608ae1be2507e8096aaa4b2c61349ac3fa4d872fa239cf0",
    "docs/gateway-reliability/native-checks/cli-msrv-unit.log": "3a422cb247711a09089b45623b6f3d546fe81795f792f709944addd29e1f11bf",
    "docs/gateway-reliability/native-checks/cli-msrv-gateway.log": "f9f9659d53bb78acad4081f9cb76d52c1680984808d6f6f0711acec1fb93645a",
    "docs/gateway-reliability/native-checks/vulture-preview-receipt.json": "622ea6fe336ecf0aa699ae26d1210c13d747c762d6eb40c30677eb3f4e8354e6",
    "docs/gateway-reliability/native-checks/cli-msrv-reference.log": "e3a2e8e38104d7f65209fa017f96db4bfcf94b945c9f2a1f2b05df004c8c8fd7",
    "docs/gateway-reliability/native-checks/cli-gateway-qualified8.log": "25a67132ad864da29b259b10bffc694a0d66609f586ee489a856dbb8becf33e3",
    "docs/gateway-reliability/native-checks/cli-clippy8.log": "0e3a7d19624aa5b6c2d3dcb52a918636584d2498f295a050d54c475a747c68aa",
    "docs/gateway-reliability/native-checks/cli-unit-review8.log": "5d9684a725ec41fa2c919e4babf63e6956725070447b07acf142fcf1cf099cdb",
    "docs/gateway-reliability/native-checks/cli-msrv-unit8.log": "278ef1102ea2c5f4154b2267562c00507cc7244bb1c2d7ebc5ba91d99860e41a",
    "docs/gateway-reliability/native-checks/cli-reference8.log": "330f99f91fc710d201b970df41a5a193ce1ed7846cffd7b77eb7efc2ee28aa9d",
    "docs/gateway-reliability/native-checks/cli-msrv-gateway8.log": "33cd6af30193d32be2a5f5eb0a934dd236768074518886ace9a9f85bf1c30dae",
    "docs/gateway-reliability/native-checks/cli-msrv-reference8.log": "bb15c39e27d464f139460b8a29bb956bfbbb795410e3529254baa7409eb1c1a4"
  }
}
```
