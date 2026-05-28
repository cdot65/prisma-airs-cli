---
"@cdot65/prisma-airs-cli": patch
---

Interpolate known Mustache-style severity placeholders (`{{CRITICAL_RISK}}`, `{{HIGH_RISK}}`, `{{MEDIUM_RISK}}`, `{{LOW_RISK}}`, `{{INFORMATIONAL_RISK}}`) in the `airs redteam report` summary text. The upstream report renderer leaks these tokens uninterpolated; the CLI now maps them to readable strings (e.g. `high risk`) at the SDK normalizer boundary. Unknown `{{...}}` tokens are left intact so future leaks remain visible.
