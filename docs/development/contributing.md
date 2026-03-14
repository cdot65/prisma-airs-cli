# Contributing

## Branch Naming

Prefix personal branches with your username:

```bash
git checkout -b cdot65/feature-name
```

## Code Style

Biome handles both linting and formatting. TypeScript strict mode is enabled project-wide.

```bash
pnpm run lint       # Check for issues
pnpm run lint:fix   # Auto-fix
pnpm run format     # Format all files
```

!!! tip "Run all checks before committing"
    ```bash
    pnpm run lint && pnpm tsc --noEmit && pnpm test
    ```

## Development Workflow

1. Fork and clone the repo
2. `pnpm install`
3. Create a feature branch (`cdot65/your-feature`)
4. Make changes
5. Run checks: `pnpm run lint && pnpm tsc --noEmit && pnpm test`
6. Commit and push
7. Open a PR against `main`

## PR Guidelines

- Keep PRs focused — one logical change per PR
- Include tests for new functionality
- Ensure all CI checks pass
- Update docs if behavior changes

!!! warning "CI must pass"
    All of these are required before merge:

    - `pnpm run lint` — Biome lint
    - `pnpm run format:check` — Biome format
    - `pnpm tsc --noEmit` — TypeScript type-check
    - `pnpm test` — Full test suite
    - Docs build (if docs changed)

## Commit Messages

Use concise messages starting with a verb:

| Prefix | When to use |
|--------|------------|
| `add` | New feature or file |
| `fix` | Bug fix |
| `update` | Enhancement to existing feature |
| `refactor` | Code restructuring, no behavior change |
| `docs` | Documentation only |
| `test` | Test additions or fixes |
