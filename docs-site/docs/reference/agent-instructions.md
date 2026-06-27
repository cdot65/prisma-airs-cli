---
title: AI Agent Instructions
---

# AI Agent Instructions

Instructions for AI coding agents (Claude Code, Gemini CLI, Cursor, etc.) to use the `airs` CLI programmatically.

The canonical file lives at [`AGENTS.md`](https://github.com/cdot65/prisma-airs-cli/blob/main/AGENTS.md) in the repository root. It is designed to be loaded into an AI agent's context window so the agent can operate the CLI autonomously.

## How to Use

### Claude Code

Add to your project's `CLAUDE.md`:

```markdown
See AGENTS.md for instructions on using the `airs` CLI.
```

Or reference it directly — Claude Code automatically reads `CLAUDE.md` and can be pointed to `AGENTS.md`.

### Gemini CLI

Add to your project's `GEMINI.md` or system instructions:

```markdown
See AGENTS.md for instructions on using the `airs` CLI.
```

### Other Agents

Copy the contents of `AGENTS.md` into your agent's system prompt or context. The file is self-contained and requires no external references.

## What's Covered

The agent instructions document covers:

- **Authentication** — which credentials are needed for which commands, how to verify them
- **Output formats** — how to use `--output json` for machine-parseable results
- **Complete command reference** — every command, flag, default, and required parameter
- **Common workflows** — step-by-step recipes for scanning, CRUD, red teaming, auditing, guardrail optimization
- **Agent loop protocol** — references `program.md` for the autonomous guardrail optimization loop
- **Error handling** — common errors and fixes
- **Platform rules** — critical AIRS behaviors agents must know (profile naming, propagation delays, etc.)
- **Config file** — location, format, and field reference

## Design Principles

The instructions are written for agents, not humans:

1. **Structured for parsing** — tables, code blocks, consistent formatting
2. **`--output json` emphasized** — agents should always request JSON for programmatic parsing
3. **Non-interactive patterns** — all commands documented with flags that bypass interactive prompts
4. **Error recovery** — common failure modes and how to handle them
5. **Credential scoping** — clear mapping of which credentials each command needs
