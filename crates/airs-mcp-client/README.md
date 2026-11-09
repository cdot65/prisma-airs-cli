# airs-mcp-client

[![crates.io](https://img.shields.io/crates/v/airs-mcp-client.svg)](https://crates.io/crates/airs-mcp-client)

The client behind `airs --mcp`: gateway sign-in (OAuth PKCE), a private session cache, typed bounded tool calls and durable operation receipts. It holds no server code, SQLite or backend credentials.

Built for the `airs` CLI. Its version moves in lockstep with it, and its API may change between releases.

Part of [airs-cli](https://github.com/cdot65/prisma-airs-cli). Documentation: <https://cdot65.github.io/prisma-airs-cli/>.
