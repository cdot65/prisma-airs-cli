---
"@cdot65/prisma-airs-cli": patch
---

Add a live smoke test reference (`docs/development/smoke-tests.md`) covering 16 read-only commands across install/version verification, runtime security, red team, and model security. The unit suite is fully mocked — zero tests hit a real AIRS tenant — so this reference is the standing checklist for catching wire-format drift after CLI releases or SDK upgrades.
