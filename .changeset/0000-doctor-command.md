---
"@cdot65/prisma-airs-cli": minor
---

Add `airs doctor` — credential and connectivity preflight. Checks Node.js version, config file presence/validity, which scanner and management credentials are set (and from which source), scanner API reachability via a quota-free results query, and management OAuth via a minimal topics call. Network checks are time-boxed at 5s with clear offline messaging. Prints a pass/warn/fail report with fix hints; `--output json|yaml` for machine-readable results. Exits 0 when healthy (warnings OK), 1 on any failure.
