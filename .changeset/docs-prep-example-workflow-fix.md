---
"@cdot65/prisma-airs-cli": patch
---

Rewrite `docs/cli/examples/README.md` to document the real CLI-docs pipeline: YAML sidecars (`docs/cli/examples/<area>.yaml`) feed `pnpm docs:gen`, which writes the auto-generated pages under `docs/cli/**`. Editing those pages by hand fails `pnpm docs:check`.
