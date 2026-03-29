# GEMINI.md

This file provides guidance to Gemini CLI when working with code in this repository.

## Project

Prisma AIRS CLI (`airs`) — a CLI and library for Palo Alto Prisma AI Runtime Security. Manages security profiles, custom topic guardrails, AI red teaming, and model security scanning.

## Key References

- `AGENTS.md` — full CLI command reference with flags, output formats, and workflows
- `program.md` — autonomous guardrail optimization protocol (the agent loop)
- `CLAUDE.md` — additional project context and conventions

## Commands

```bash
pnpm install        # install deps
pnpm build          # tsc compile
pnpm test           # vitest
pnpm lint           # biome check
pnpm lint:fix       # biome fix
```

## Guardrail Optimization Loop

For autonomous custom topic guardrail optimization, follow the protocol in `program.md`. It covers setup, baseline, the iteration loop, revert procedure, plateau detection, and companion topics.

Key commands: `topics create`, `topics apply`, `topics eval`, `topics revert`, `topics sample`, `topics get --output json`.

## Conventions

- TypeScript ESM with `.js` extensions in imports
- Biome formatter (100-char line width)
- Tests in `tests/unit/` mirroring `src/` structure
- Always use `--format json` when parsing CLI output programmatically
