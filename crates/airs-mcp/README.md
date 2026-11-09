# airs-mcp

The optional, protected MCP service for Prisma AIRS history, collection and reports. It runs behind an MCP gateway and ships as a container image, not on crates.io.

```sh
cargo test -p airs-mcp
```

To deploy it, see [`deploy/airs-mcp`](../../deploy/airs-mcp) and the [runbook](../../docs/RUNTIME_MCP_DEPLOYMENT_RUNBOOK.md). The contract is in [`docs/RUNTIME_MCP_CONTRACT.md`](../../docs/RUNTIME_MCP_CONTRACT.md).
