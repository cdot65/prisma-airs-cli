# Testing

## Running Tests

```bash
pnpm test              # All unit tests (vitest run)
pnpm run test:watch    # Watch mode
pnpm run test:coverage # Coverage report (V8 provider)
pnpm run test:e2e      # E2E tests (opt-in, requires real Vertex AI creds)
pnpm tsc --noEmit      # Type-check (strict mode, src/ only)
```

!!! info "No AIRS credentials needed for the unit suite"
    Unit tests never instantiate the `@cdot65/prisma-airs-sdk` clients. They inject mock service implementations directly into the code paths under test, so the SDK and its HTTP layer are bypassed entirely. You can run `pnpm test` with no AIRS env vars set.

## Test Structure

```
tests/
├── unit/                  34 spec files
│   ├── airs/              management.spec.ts, modelsecurity.spec.ts, promptsets.spec.ts,
│   │                      redteam.spec.ts, runtime.spec.ts, scanner.spec.ts
│   ├── audit/             evaluator.spec.ts, report.spec.ts, runner.spec.ts
│   ├── backup/            io.spec.ts
│   ├── cli/               backup-renderer.spec.ts, backup.spec.ts, bulk-scan-state.spec.ts,
│   │                      parse-input.spec.ts, profile-builder.spec.ts, profiles-cleanup.spec.ts,
│   │                      redteam-init.spec.ts, restore.spec.ts, topics-apply.spec.ts,
│   │                      topics-create.spec.ts, topics-eval.spec.ts, topics-revert.spec.ts,
│   │                      topics-sample.spec.ts
│   ├── config/            loader.spec.ts, schema.spec.ts
│   ├── core/              constraints.spec.ts, metrics.spec.ts, prompt-loader.spec.ts
│   └── llm/               prompts.spec.ts, provider.spec.ts, schemas.spec.ts, service.spec.ts
├── e2e/                   vertex-provider.e2e.spec.ts (opt-in, real Vertex AI creds)
└── helpers/               mocks.ts
```

## Mocking

Mocks are defined in [`tests/helpers/mocks.ts`](https://github.com/cdot65/prisma-airs-cli/blob/main/tests/helpers/mocks.ts) as **service-level factories**, not HTTP-level interceptors. Each factory returns an in-memory implementation of one of the project's service interfaces from [`src/airs/types.ts`](https://github.com/cdot65/prisma-airs-cli/blob/main/src/airs/types.ts):

| Factory | Returns | Used to test |
|---|---|---|
| `createMockManagementService()` | `ManagementService` | Topic and profile CRUD code paths |
| `createMockScanService()` | `ScanService` | Synchronous scanning, with optional regex `triggerPatterns` |
| `createMockAllowScanService()` | `ScanService` | Allow-intent scanner behavior (matching prompts → `topic_violation: true`) |
| `createMockRedTeamService()` | `RedTeamService` | Red team scan and target operations |
| `createMockModelSecurityService()` | `ModelSecurityService` | Model security groups, rules, scans |

The unit suite injects these mocks directly into the code under test via constructor or function arguments — the SDK and its `Scanner` / `ManagementClient` / `RedTeamClient` / `ModelSecurityClient` are never instantiated. Because no HTTP requests ever leave the process, no AIRS credentials are required.

E2E tests under `tests/e2e/` are different: they exercise the LangChain Vertex AI provider for the `audit` command's LLM path against real Google Cloud, gated by `pnpm test:e2e`. They do **not** cover the AIRS Scanner, Management, RedTeam, or ModelSecurity APIs. For live AIRS coverage, run the manual checklist in [Live Smoke Tests](smoke-tests.md) — that's the authoritative way to catch backend wire-format drift, especially under SDK 0.8.0's now-on runtime Zod validation.

## Coverage

Coverage is collected via **V8** and excludes files that aren't meaningfully testable:

| Excluded pattern | Why |
|-----------------|-----|
| `src/cli/**` | Interactive UI (prompts, rendering) |
| `src/index.ts` | Re-exports only |
| `**/types.ts` | Type-only files, no runtime code |

Thresholds (from `vitest.config.ts`): **90% lines, 95% functions, 80% branches, 90% statements.**

```bash
pnpm run test:coverage
```

## Running Specific Tests

Single file:

```bash
pnpm test -- tests/unit/core/metrics.spec.ts
```

By name pattern:

```bash
pnpm test -- -t "computes coverage as min of TPR and TNR"
```

!!! tip "Watch a single file"
    ```bash
    pnpm run test:watch -- tests/unit/core/metrics.spec.ts
    ```
