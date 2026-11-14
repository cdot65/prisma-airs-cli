# Gateway reliability native checks: prisma-airs-cli

The subsequent [CLI9.1.0 registry integration](INTEGRATION_9_1_0.md) has its own
source-bound results, including the MCP macOS storage refusals; the original
checks below retain their earlier source identity.

The owner supplied the test environment in this session. These new local checks
used isolated source copies, cleared inherited environments, locked dependencies
and synthetic/loopback fixtures on macOS ARM64. They do not relabel historical
CI or establish live-service, fleet or release acceptance. No real tenant config
or credentials were copied into the test source snapshots.

| Rust | Executed command | Result | Raw log |
| --- | --- | --- | --- |
| 1.99.0 | `cargo test --locked --offline -p airs-cli --test gateway_writes -- --test-threads=1` | 80 passed | [log](native-checks/cli-gateway9.log) |
| 1.89.0 | `cargo test --locked --offline -p airs-cli --test gateway_writes -- --test-threads=1` | 80 passed | [log](native-checks/cli-msrv-gateway9.log) |
| 1.99.0 | `cargo test --locked --offline -p airs-cli --lib gateway:: -- --test-threads=1` | 87 passed | [log](native-checks/cli-unit9.log) |
| 1.89.0 | `cargo test --locked --offline -p airs-cli --lib gateway:: -- --test-threads=1` | 87 passed | [log](native-checks/cli-msrv-unit9.log) |
| 1.99.0 | `cargo test --locked --offline -p airs-runtime-core --features test-support --lib debug::tests:: -- --test-threads=1` | 15 passed | [log](native-checks/cli-debug9.log) |
| 1.89.0 | `cargo test --locked --offline -p airs-runtime-core --features test-support --lib debug::tests:: -- --test-threads=1` | 15 passed | [log](native-checks/cli-msrv-debug9.log) |
| 1.99.0 | `cargo test --locked --offline -p airs-cli --test cli_reference` | 2 passed | [log](native-checks/cli-reference9.log) |
| 1.89.0 | `cargo test --locked --offline -p airs-cli --test cli_reference` | 2 passed | [log](native-checks/cli-msrv-reference9.log) |
| 1.99.0 | `cargo clippy --locked --offline -p airs-cli --lib --test gateway_writes -- -D warnings` | passed; no warnings | [log](native-checks/cli-clippy9.log) |

[Exact tested source, compiler, retained binary and log hashes](native-checks/evidence.json)
bind these results. The final candidate binaries were copied into separate
qualified-artifact paths so later builds cannot overwrite their bindings.

Initial native runs found and resolved a CLI error conversion, an isolated
dependency-cache miss, feature-off SDK warnings and the artificial deep-fixture
test-thread stack assumption. The final suites above pass.

Tooling side effect: the isolated Rust1.89 installation auto-updated the shared
rustup launcher from 1.29.1 to 1.29.3. No CLI installation was replaced.

Other platforms, actual service contracts and SDK-to-CLI getter adoption remain
separate work. No package publication or fleet admission is implied.

[Owner-provided vulture local-preview receipt](native-checks/vulture-preview-receipt.json)
records an additional candidate dry-run through the real registered local context,
using a synthetic resource ID and sending no request. It is not service acceptance.
