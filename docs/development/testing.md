# Testing

## Running Tests

```bash
pnpm test              # All unit/integration tests
pnpm run test:watch    # Watch mode
pnpm run test:coverage # Coverage report (v8 provider)
pnpm run test:e2e      # E2E tests (requires real creds)
pnpm tsc --noEmit      # Type-check (strict mode)
```

## Test Structure

Tests live in `tests/`, mirroring the `src/` layout:

```
tests/
├── unit/                  28 spec files
│   ├── airs/              scanner.spec.ts, management.spec.ts, modelsecurity.spec.ts, promptsets.spec.ts, redteam.spec.ts, runtime.spec.ts
│   ├── audit/             evaluator.spec.ts, runner.spec.ts, report.spec.ts
│   ├── cli/               parse-input.spec.ts, bulk-scan-state.spec.ts
│   ├── config/            schema.spec.ts, loader.spec.ts
│   ├── core/              loop.spec.ts, metrics.spec.ts, constraints.spec.ts
│   ├── llm/               provider.spec.ts, schemas.spec.ts, service.spec.ts, prompts.spec.ts
│   ├── memory/            store.spec.ts, extractor.spec.ts, injector.spec.ts, diff.spec.ts, prompts.spec.ts
│   ├── persistence/       store.spec.ts
│   └── report/            json.spec.ts, html.spec.ts
├── integration/           loop.integration.spec.ts
├── e2e/                   vertex-provider.e2e.spec.ts (opt-in)
└── helpers/               mocks.ts
```

## Mocking

**MSW** (Mock Service Worker) intercepts all HTTP requests — no real AIRS credentials needed for unit or integration tests.

!!! info "No credentials needed"
    All HTTP calls to AIRS and LLM APIs are intercepted by MSW handlers in `tests/helpers/mocks.ts`. You can run the full unit/integration suite without any API keys.

E2E tests require real Vertex AI credentials and are opt-in via separate config.

## Coverage

Coverage is collected via **V8** and excludes files that aren't meaningfully testable:

| Excluded pattern | Why |
|-----------------|-----|
| `src/cli/**` | Interactive UI (prompts, rendering) |
| `src/index.ts` | Re-exports only |
| `**/types.ts` | Type-only files, no runtime code |

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
