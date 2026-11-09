# airs-cli

[![crates.io](https://img.shields.io/crates/v/airs-cli.svg)](https://crates.io/crates/airs-cli)

`airs` is a native command-line tool for Prisma AIRS.

Version 9.0.0 covers runtime security, AI red teaming, model security, DLP, the AI gateway and
supply chain scanning.

## Install

**[Cargo](https://crates.io/crates/airs-cli)** (Rust 1.89 or newer):

```sh
cargo install --locked airs-cli
```

**[npm](https://www.npmjs.com/package/@cdot65/prisma-airs-cli)** (a simplified install with prebuilt binaries; Apple
Silicon macOS, Linux x64 and arm64; Node 18 or newer):

```sh
npm install -g @cdot65/prisma-airs-cli
```

Then check it and register a tenant:

```sh
airs --version     # prints the version, e.g. 9.0.0
airs tenant create <name>
airs doctor
```

## Documentation

Everything else is at **<https://cdot65.github.io/prisma-airs-cli/>**.
