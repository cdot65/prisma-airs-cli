---
"@cdot65/prisma-airs-cli": patch
---

`airs redteam report <jobId> --attacks --severity X` now prints a footnote when the `list-attacks` endpoint returns fewer items than the summary severity breakdown promised for that severity, so operators know the discrepancy is upstream (server-side aggregator divergence) rather than a CLI bug. `service.listAttacks()` return shape changes from `RedTeamAttack[]` to `{ attacks: RedTeamAttack[]; totalItems?: number }` to surface pagination — only internal callers affected.
