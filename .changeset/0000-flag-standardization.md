---
"@cdot65/prisma-airs-cli": minor
---

Standardized flags across every command group: `--output` always means format, file destinations are `--output-file`, input files are `--file`, pagination is `--limit`/`--offset`, and destructive bypass is `--force`. All old spellings keep working in v2 (hidden, with a stderr deprecation notice) and will be removed in v3 — see the Flag Migration guide. Also added `--output pretty|json|yaml` to redteam prompts/instances/registry-credentials and client-side `--limit`/`--offset` to redteam list commands.
