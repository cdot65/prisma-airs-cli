# Copilot Instructions — Prisma AIRS CLI

This file provides guidance to GitHub Copilot, Codex, Pi, and other AI coding agents.

## Project

Prisma AIRS CLI (`airs`) — a CLI and library for Palo Alto Prisma AI Runtime Security. Manages security profiles, custom topic guardrails, AI red teaming, and model security scanning.

## Key References

- `AGENTS.md` — full CLI command reference with flags, output formats, and workflows
- `program.md` — autonomous guardrail optimization protocol (the agent loop)
- `CLAUDE.md` — additional project context and conventions

## Guardrail Optimization Loop

For autonomous custom topic guardrail optimization, follow the protocol in `program.md`. It covers setup, baseline, the iteration loop, revert procedure, plateau detection, and companion topics.

Key commands: `topics create`, `topics apply`, `topics eval`, `topics revert`, `topics sample`, `topics get --output json`.

## Conventions

- TypeScript ESM with `.js` extensions in imports
- Biome formatter (100-char line width)
- Tests in `tests/unit/` mirroring `src/` structure
- Always use `--format json` when parsing CLI output programmatically
