---
"@cdot65/prisma-airs-cli": patch
---

Rewrite `docs/development/testing.md` to accurately describe the test suite. The page previously claimed MSW intercepted HTTP requests and listed test directories that don't exist (`memory/`, `persistence/`, `integration/`, `report/`). The unit suite actually injects in-memory service mocks from `tests/helpers/mocks.ts` — the SDK and its HTTP layer are never instantiated. Also removes the unused `msw` devDependency and links the page to the new live AIRS smoke test reference for API coverage that the unit suite intentionally does not provide.
