---
"@cdot65/prisma-airs-cli": minor
---

Machine-readable output is now pipe-safe: `--output json|yaml|csv` emits only the payload on stdout, with progress, banners, and rate-limit warnings moved to stderr. Exit codes are standardized (0 success, 1 runtime/API failure, 2 usage error) across every command group. API errors show the HTTP status and a `--debug` hint. Async command errors are handled properly via parseAsync.
