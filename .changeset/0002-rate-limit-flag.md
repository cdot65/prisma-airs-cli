---
"@cdot65/prisma-airs-cli": patch
---

Added `--rate <n>` flag to guardrail generate and resume commands that caps AIRS scan API calls to N per second, preventing rate limit errors during intensive scan loops.
