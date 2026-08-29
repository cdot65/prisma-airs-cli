# Local Setup

## Prerequisites

| Requirement | Version |
|-------------|---------|
| Node.js | >= 20.0.0 (ES2022 target) |
| pnpm | Latest (recommended) |

## Clone and Install

```bash
git clone git@github.com:cdot65/prisma-airs-cli.git
cd prisma-airs-cli
pnpm install
```

## Environment

```bash
cp .env.example .env
```

Edit `.env` with your credentials:

| Variable | Required | What it's for |
|----------|:--------:|-------------|
| `PANW_AI_SEC_API_KEY` | Yes | Prisma AIRS Scan API key |
| `PANW_MGMT_CLIENT_ID` | Yes | AIRS Management OAuth2 client ID |
| `PANW_MGMT_CLIENT_SECRET` | Yes | AIRS Management OAuth2 client secret |
| `PANW_MGMT_TSG_ID` | Yes | Tenant Service Group ID |

:::note[Tests run without credentials]
Unit and integration tests use MSW mocks — you only need real credentials for actual AIRS operations.
:::

## Register `airs` command

To make the `airs` binary available globally from your source checkout:

```bash
pnpm run build
pnpm link --global
airs --version   # 4.0.0
```

After making code changes, re-run `pnpm run build` for the linked `airs` command to reflect them. `pnpm run dev` doesn't require a build step.

## Development Commands

| Command | What it does |
|---------|-------------|
| `pnpm run dev` | Run CLI via tsx — no build needed (e.g. `pnpm run dev runtime scan ...`) |
| `pnpm local:setup` | Install dependencies and build the local package |
| `pnpm local:smoke` | Build, print the compiled version, and render compiled help |
| `pnpm local:dev:help` | Render help directly from TypeScript source |
| `pnpm local:dev:profiles` | List profiles as JSON using the source entry point |
| `pnpm local:dev:topics` | Traverse topics and render YAML using the source entry point |
| `pnpm local:dev:doctor` | Run doctor with Markdown output using the source entry point |
| `pnpm local:link` / `pnpm local:unlink` | Build and globally link, or remove the global package |
| `pnpm run build` | Compile TypeScript to `dist/` |
| `pnpm test` | Run all tests |
| `pnpm run test:watch` | Watch mode |
| `pnpm run test:coverage` | Coverage report |
| `pnpm run lint` | Biome lint check |
| `pnpm run lint:fix` | Auto-fix lint issues |
| `pnpm run format` | Format with Biome |
| `pnpm run format:check` | Check formatting (no write) |
| `pnpm tsc --noEmit` | Type-check |

## Data Directories

Runtime data lives under `~/.prisma-airs/`:

| Path | What's in it |
|------|-------------|
| `~/.prisma-airs/runs/` | Persisted run states (JSON) |
| `~/.prisma-airs/memory/` | Cross-run learning store |
| `~/.prisma-airs/config.json` | Optional config file |

:::info[Config priority]
CLI flags > environment variables > config file > Zod schema defaults
:::

:::note[pnpm 10 argument forwarding]
Invoke arbitrary development commands without a standalone separator:
`pnpm dev --help` and `pnpm dev runtime profiles list --output json`. A
standalone `--` is forwarded literally to Commander by pnpm 10.
:::

## Verify Setup

Run all three checks to confirm everything works:

```bash
pnpm test           # All tests pass (no AIRS creds needed)
pnpm run lint       # No lint errors
pnpm tsc --noEmit   # No type errors
```

:::tip[All three should pass on a fresh clone]
If any fail, make sure you're on Node >= 20 and have run `pnpm install`.
:::
