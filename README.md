# airs-cli

[![Docs](https://img.shields.io/badge/docs-prisma--airs--cli-blue)](https://cdot65.github.io/prisma-airs-cli/)
[![airs-cli](https://img.shields.io/crates/v/airs-cli.svg?label=airs-cli)](https://crates.io/crates/airs-cli)
[![airs-runtime-core](https://img.shields.io/crates/v/airs-runtime-core.svg?label=airs-runtime-core)](https://crates.io/crates/airs-runtime-core)
[![airs-mcp-client](https://img.shields.io/crates/v/airs-mcp-client.svg?label=airs-mcp-client)](https://crates.io/crates/airs-mcp-client)

`airs` is one native command line to manage, scan and report on everything in Prisma AIRS.
Version 9.1.0 covers runtime security, AI red teaming, model security, DLP, the AI gateway and supply chain, and an [optional MCP service](https://cdot65.github.io/prisma-airs-cli/mcp-server/introduction) adds retained history, scheduled collection and reports for you and your AI agents.

Release availability is recorded on [Forgejo](https://git.cdot.io/cdot/prisma-airs-cli/releases)
and in the [release notes](docs/RELEASE-NOTES/9.1.0.md).

## Install

Pick one, then see the [full install guide](https://cdot65.github.io/prisma-airs-cli/getting-started/install) for other channels.

**[Cargo](https://crates.io/crates/airs-cli)** (Rust 1.89 or newer):

```sh
cargo install --locked airs-cli
```

**[npm](https://www.npmjs.com/package/@cdot65/prisma-airs-cli)** (prebuilt binaries for Apple Silicon macOS and Linux x64 and arm64; Node 18 or newer):

```sh
npm install -g @cdot65/prisma-airs-cli
```

Then check it and register a tenant:

```sh
airs --version     # prints the version, e.g. 9.1.0
airs tenant create <name>
airs doctor
```

## How it works

With just the CLI:

```mermaid
flowchart LR
  User(["You"]) --> CLI["airs CLI"]
  CLI --> Tenants[("Tenant store on your machine")]
  CLI -->|"API calls with your tenant"| AIRS["Prisma AIRS"]
  CLI --> Output["Tables, JSON and HTML reports"]
```

With the optional MCP service added:

```mermaid
flowchart LR
  User(["You"]) --> CLI["airs CLI"]
  Agent(["AI agent, such as Claude Code"]) --> Gateway
  CLI --> Tenants[("Tenant store on your machine")]
  CLI -->|"everyday commands"| AIRS["Prisma AIRS"]
  CLI -->|"airs --mcp"| Gateway["MCP gateway"]
  Gateway -->|"sign-in"| IdP["Identity provider"]
  Gateway -->|"authorized requests"| Service["airs-mcp service"]
  Service -->|"scheduled collection"| AIRS
  Service --> Archive[("Encrypted history archive")]
  Service --> Reports["Signal Brief reports"]
```
