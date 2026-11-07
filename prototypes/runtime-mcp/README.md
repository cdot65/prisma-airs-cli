# Isolated runtime MCP feasibility workspace (M0)

M0.2 prototypes and the M0.6 disposable spike for the optional runtime MCP service
([contract](../../docs/RUNTIME_MCP_CONTRACT.md),
[feasibility record](../../docs/RUNTIME_MCP_M0_FEASIBILITY.md),
[M0.6 runbook](../../docs/RUNTIME_MCP_M0_6_RUNBOOK.md)). Not linked into `airs`;
not a root workspace member. No AIRS credentials, tenants or customer data; test
keys under `crates/airs-mcp-spike/tests/fixtures/` are synthetic.

```sh
cargo +1.89.0 fmt --all --check --manifest-path prototypes/runtime-mcp/Cargo.toml
cargo +1.89.0 clippy --locked --all-targets --manifest-path prototypes/runtime-mcp/Cargo.toml -- -D warnings
cargo +1.89.0 test --locked --manifest-path prototypes/runtime-mcp/Cargo.toml -- --nocapture
cargo package -p airs-mcp-client --manifest-path prototypes/runtime-mcp/Cargo.toml --allow-dirty   # core: crates/airs-runtime-core (M1)
```

The skeleton crates use the approved package names only to qualify name/package/
feature behavior; M1 creates the production crates in the root workspace. Nothing
here is published. `deploy/m0.6/` holds the spike image and Kubernetes templates;
they contain placeholders and must be copied into talos-cluster by the user.
