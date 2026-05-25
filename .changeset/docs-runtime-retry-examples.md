---
"@cdot65/prisma-airs-cli": patch
---

docs(cli): live examples for `runtime api-keys create` and `runtime api-keys regenerate` against a fresh-tenant retry. Burns two entries off the docs allowlist. Re-tags remaining runtime allowlist entries to new tracking issues: `runtime api-keys delete` and `runtime customer-apps delete` against new SDK bugs (server-side succeeds, SDK schema rejects string-body 200), and `runtime customer-apps update` against a new upstream 503.
