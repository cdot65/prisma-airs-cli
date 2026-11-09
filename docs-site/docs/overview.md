---
title: Overview
sidebar_position: 1
---

# Prisma AIRS CLI

`airs` is a native command line for Palo Alto Networks Prisma AIRS. It is a single
self-contained executable, written in Rust on the published
[`scm-sdk`](https://cdot65.github.io/scm-sdk-rs/) crate, and it replaces the TypeScript
Prisma AIRS CLI (7.x). From 8.0.0 it is published from this repository on every channel:
Homebrew, npm, crates.io and prebuilt release tarballs.

It starts in about 6 ms where the TypeScript CLI took 81 ms, spends 5.5x less time of its own
on a scan, and installs as one 31 MiB executable instead of 220 MiB of Node.js and npm
packages (see [Performance](./performance.md)).

It covers:

| Area | Commands |
|---|---|
| Tenants and diagnostics | [`airs tenant`](./commands/tenant.md), [`airs doctor`](./commands/doctor.md) |
| Runtime security | [`airs runtime`](./commands/runtime.md): security profiles, custom topics, prompt scans, bulk scans, API keys, customer applications, dashboards, sessions, the environment report, profile backup and restore |
| Data loss prevention | [`airs runtime dlp`](./commands/runtime-dlp.md): patterns, data profiles, dictionaries, filtering profiles, backup and restore |
| AI Red Teaming | [`airs redteam`](./commands/redteam.md): scans, targets, prompt sets, adapters, the network broker, reports, target backup and restore |
| AI Model Security | [`airs model-security`](./commands/model-security.md): security groups, rules, scans, models, labels, the PyPI index and `install` |
| AI Gateway | [`airs gateway`](./commands/gateway.md): workspaces, scopes, telemetry, inventory, writes, inference and the report |
| AI Supply Chain | [`airs supplychain`](./commands/supplychain.md): scans, findings, statistics, rules and the report |

## Want history, schedules and reports?

The optional [MCP server](./mcp-server/introduction.md) collects your scan activity on a schedule,
keeps it in an encrypted archive and answers questions about it from the CLI or an AI agent.

## Names

The command is `airs`. Two more names run the same program and print the same output:

- `airs-cli`, the TypeScript CLI's command name, so scripts written for 7.x keep finding a
  command;
- `airs-cli-rs`, the name of the Rust CLI before 1.0.

Help texts, hints and examples always say `airs`. See [Install](./getting-started/install.md)
for how each channel provides the names.

## Principles

- **Tenants are the only configuration.** Credentials and endpoints come from the selected
  tenant's file in the `airs` store, never from environment variables
  ([Configuration and tenants](./getting-started/configuration.md)).
- **stdout is data.** Status lines and errors go to stderr; `--quiet` hides the status
  lines ([Output, exit codes and the debug log](./getting-started/output-and-exit-codes.md)).
- **Fail closed.** A scan that fails, times out or returns an unknown verdict is never shown
  as allowed, and an incomplete listing is never reported as complete.
- **Secrets stay secret.** No output prints a credential, token, raw response body or URL
  query; writes that return a one-time credential put it only in a new private file.
- **Writes are sent once.** A write whose outcome is unknown is reported as unknown, never
  re-sent.

## Where to go next

1. [Install `airs`](./getting-started/install.md).
2. Coming from the TypeScript CLI? Read
   [Upgrading from the TypeScript CLI (7.x)](./getting-started/upgrading-from-typescript.md).
3. [Create or import a tenant](./getting-started/configuration.md), then run `airs doctor`.
4. Look up any command in the [command reference](./reference/commands/index.md).
