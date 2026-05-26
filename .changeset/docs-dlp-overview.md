---
"@cdot65/prisma-airs-cli": patch
---

docs(dlp): add DLP Overview page so the section matches the sidenav pattern used by Guardrail Optimization and Runtime Security peers. Covers the 20-command surface, auth (incl. `PANW_DLP_ENDPOINT`), resource model, shared merge-patch UX (`--set` / `--clear` / `--body-file`, per-resource required fields), and common gotchas (soft-delete idiom, multipart `dictionaries` create/replace, 200/204 region fallback, `profiles delete` stub exiting 2). Adds the `delete` row + section to `profiles.md` documenting the stub. Adds `PANW_DLP_ENDPOINT` to `reference/environment-variables.md`. Cross-links DLP from the Runtime Security overview grid.
