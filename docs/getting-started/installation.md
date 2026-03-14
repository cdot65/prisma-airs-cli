---
title: Installation
---

# Installation

Get Prisma AIRS CLI running in under 5 minutes. Choose between npm (recommended) or Docker.

## Prerequisites

Before you begin, make sure you have:

- **Node.js 20+** (LTS recommended) — check with `node -v`
- **Prisma AIRS access** — scan API key + management API OAuth2 credentials
- **LLM provider credentials** — at least one supported provider ([see options](configuration.md#llm-providers))

## Install from npm

=== "npm (recommended)"

    ```bash
    npm install -g @cdot65/prisma-airs-cli
    ```

=== "pnpm"

    ```bash
    pnpm add -g @cdot65/prisma-airs-cli
    ```

    !!! warning "pnpm global bin PATH"
        pnpm's global bin directory is often not in your `PATH` by default. If `airs` is not found after install, run:
        ```bash
        pnpm setup
        ```
        Then restart your terminal. This adds pnpm's global bin to your shell profile.

Verify the installation:

```bash
$ airs --version
1.0.1

$ airs --help
Usage: airs [options] [command]

CLI and library for Palo Alto Prisma AIRS — guardrail refinement, AI red
teaming, model security scanning, profile audits

Options:
  -V, --version   output the version number
  -h, --help      display help for command

Commands:
  runtime         Runtime prompt scanning against AIRS profiles
  redteam         AI Red Team scan operations
  model-security  AI Model Security operations — groups, rules, scans
  help [command]  display help for command
```

!!! tip "Try without installing"
    Run once without a global install:
    ```bash
    npx @cdot65/prisma-airs-cli runtime topics generate
    ```

## Set Up Credentials

Prisma AIRS CLI needs credentials for both the LLM provider and AIRS APIs. The fastest way is an `.env` file or shell exports.

=== "macOS / Linux"

    ```bash
    export ANTHROPIC_API_KEY=sk-ant-...
    export PANW_AI_SEC_API_KEY=your-scan-api-key
    export PANW_MGMT_CLIENT_ID=your-client-id
    export PANW_MGMT_CLIENT_SECRET=your-client-secret
    export PANW_MGMT_TSG_ID=your-tsg-id
    ```

=== "Windows (PowerShell)"

    ```powershell
    $env:ANTHROPIC_API_KEY = "sk-ant-..."
    $env:PANW_AI_SEC_API_KEY = "your-scan-api-key"
    $env:PANW_MGMT_CLIENT_ID = "your-client-id"
    $env:PANW_MGMT_CLIENT_SECRET = "your-client-secret"
    $env:PANW_MGMT_TSG_ID = "your-tsg-id"
    ```

    !!! note "Windows path length"
        If you encounter `ENAMETOOLONG` errors, enable long paths:
        ```powershell
        # Run as Administrator
        New-ItemProperty -Path "HKLM:\SYSTEM\CurrentControlSet\Control\FileSystem" `
          -Name "LongPathsEnabled" -Value 1 -PropertyType DWORD -Force
        ```

=== "Windows (cmd)"

    ```cmd
    set ANTHROPIC_API_KEY=sk-ant-...
    set PANW_AI_SEC_API_KEY=your-scan-api-key
    set PANW_MGMT_CLIENT_ID=your-client-id
    set PANW_MGMT_CLIENT_SECRET=your-client-secret
    set PANW_MGMT_TSG_ID=your-tsg-id
    ```

All five credential values are required for a functional run. The LLM key depends on your chosen provider — see the [provider table](configuration.md#llm-providers) for alternatives to `ANTHROPIC_API_KEY`.

---

## Docker

No Node.js required — just Docker. The multi-arch image supports both amd64 (Intel) and arm64 (Apple Silicon, Graviton).

First, create a `.env` file with your credentials:

```dotenv title=".env"
ANTHROPIC_API_KEY=sk-ant-...
PANW_AI_SEC_API_KEY=your-scan-api-key
PANW_MGMT_CLIENT_ID=your-client-id
PANW_MGMT_CLIENT_SECRET=your-client-secret
PANW_MGMT_TSG_ID=your-tsg-id
```

Then run the CLI:

```bash
docker run --rm --env-file .env \
  -v ~/.prisma-airs:/root/.prisma-airs \
  ghcr.io/cdot65/prisma-airs-cli runtime topics generate \
  --profile my-security-profile \
  --topic "Block phishing attempts" \
  --intent block
```

The `-v` mount persists run state and learnings between containers.

!!! tip "Shell alias"
    Add to your `.bashrc` / `.zshrc` for convenience:
    ```bash
    alias airs='docker run --rm --env-file .env -v ~/.prisma-airs:/root/.prisma-airs ghcr.io/cdot65/prisma-airs-cli'
    ```
    Then use `airs runtime topics generate`, `airs runtime topics runs`, etc.

---

## Where Data Lives

Prisma AIRS CLI stores everything under `~/.prisma-airs/`:

| Path | What's in it |
|------|-------------|
| `~/.prisma-airs/config.json` | Your persistent settings |
| `~/.prisma-airs/runs/` | Saved run states (one JSON per run) |
| `~/.prisma-airs/memory/` | Cross-run learnings (one JSON per topic category) |

On Windows, `~` resolves to `%USERPROFILE%` (typically `C:\Users\<username>`).

---

## Install from Source

For development or contributing:

```bash
git clone git@github.com:cdot65/prisma-airs-cli.git
cd prisma-airs-cli
pnpm install
cp .env.example .env
```

Requires **pnpm >= 8** (`corepack enable` to install).

### Running from source

=== "Development (tsx, no build)"

    ```bash
    pnpm run dev runtime topics generate
    ```

=== "Production (compiled)"

    ```bash
    pnpm run build
    node dist/cli/index.js runtime topics generate
    ```

### Register the `airs` command locally

To make the `airs` command available in your terminal from a source checkout:

```bash
pnpm run build
pnpm link --global
```

Then `airs --version`, `airs runtime scan`, etc. work anywhere. Changes require re-running `pnpm run build` to take effect.

### Verify setup

```bash
pnpm test          # All tests (no AIRS creds needed)
pnpm run lint      # Lint check
pnpm tsc --noEmit  # Type check
```
