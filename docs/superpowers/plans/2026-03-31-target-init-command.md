# Target Init Command Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `airs redteam targets init <provider>` command that scaffolds a target config JSON from AIRS provider templates.

**Architecture:** Single new command registered on the existing `targets` Commander group in `redteam.ts`. Calls existing `service.getTargetTemplates()`, wraps the provider blob in a `RedTeamTargetCreateRequest` skeleton, writes JSON to disk. Two unit tests validate scaffold shape and invalid-provider error.

**Tech Stack:** Commander.js, node:fs, node:path, vitest

---

### File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `src/cli/commands/redteam.ts` | Modify (lines 940-953, insert before `templates` command) | Register `init` command |
| `tests/unit/cli/redteam-init.spec.ts` | Create | Unit tests for init scaffold logic |

---

### Task 1: Write failing tests for init scaffold logic

**Files:**
- Create: `tests/unit/cli/redteam-init.spec.ts`

- [ ] **Step 1: Write the failing tests**

```ts
import { describe, expect, it } from 'vitest';

/** Valid provider names for target templates. */
const VALID_PROVIDERS = [
  'OPENAI',
  'HUGGING_FACE',
  'DATABRICKS',
  'BEDROCK',
  'REST',
  'STREAMING',
  'WEBSOCKET',
];

/**
 * Build a target config scaffold from a provider template.
 * Extracted as a pure function so it can be tested without CLI/service wiring.
 */
function buildTargetScaffold(
  provider: string,
  templates: Record<string, unknown>,
): Record<string, unknown> {
  const key = provider.toUpperCase();
  if (!VALID_PROVIDERS.includes(key)) {
    throw new Error(
      `Unknown provider "${provider}". Valid providers: ${VALID_PROVIDERS.join(', ')}`,
    );
  }
  return {
    name: '',
    target_type: key,
    connection_params: templates[key] ?? {},
    background: {},
    additional_context: {},
    metadata: {},
  };
}

describe('buildTargetScaffold', () => {
  const mockTemplates: Record<string, unknown> = {
    OPENAI: { api_key: '', model: 'gpt-4', endpoint: 'https://api.openai.com/v1' },
    HUGGING_FACE: { token: '', model_id: '' },
    DATABRICKS: { host: '', token: '' },
    BEDROCK: { region: '', model_id: '' },
    REST: { url: '', method: 'POST' },
    STREAMING: { url: '', method: 'POST' },
    WEBSOCKET: { url: '' },
  };

  it('builds correct scaffold for valid provider', () => {
    const result = buildTargetScaffold('openai', mockTemplates);
    expect(result).toEqual({
      name: '',
      target_type: 'OPENAI',
      connection_params: mockTemplates.OPENAI,
      background: {},
      additional_context: {},
      metadata: {},
    });
  });

  it('handles case-insensitive provider input', () => {
    const result = buildTargetScaffold('Hugging_Face', mockTemplates);
    expect(result.target_type).toBe('HUGGING_FACE');
  });

  it('throws on invalid provider with list of valid providers', () => {
    expect(() => buildTargetScaffold('azure', mockTemplates)).toThrow(
      'Unknown provider "azure". Valid providers: OPENAI, HUGGING_FACE, DATABRICKS, BEDROCK, REST, STREAMING, WEBSOCKET',
    );
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `pnpm test -- tests/unit/cli/redteam-init.spec.ts -v`
Expected: PASS (tests define `buildTargetScaffold` inline — this verifies the logic contract before extracting to src)

- [ ] **Step 3: Commit**

```bash
git add tests/unit/cli/redteam-init.spec.ts
git commit -m "test: add unit tests for target init scaffold logic"
```

---

### Task 2: Extract `buildTargetScaffold` and `VALID_PROVIDERS` into redteam.ts

**Files:**
- Modify: `src/cli/commands/redteam.ts` (before `registerTargetCommands`, around line 740)
- Modify: `tests/unit/cli/redteam-init.spec.ts` (import from src instead of inline)

- [ ] **Step 1: Add the exported helper to redteam.ts**

Add before the `registerTargetCommands` function (around line 740):

```ts
/** Valid provider names for target init templates. */
export const VALID_TARGET_PROVIDERS = [
  'OPENAI',
  'HUGGING_FACE',
  'DATABRICKS',
  'BEDROCK',
  'REST',
  'STREAMING',
  'WEBSOCKET',
] as const;

/** Build a target config scaffold from a provider template. */
export function buildTargetScaffold(
  provider: string,
  templates: Record<string, unknown>,
): Record<string, unknown> {
  const key = provider.toUpperCase();
  if (!VALID_TARGET_PROVIDERS.includes(key as (typeof VALID_TARGET_PROVIDERS)[number])) {
    throw new Error(
      `Unknown provider "${provider}". Valid providers: ${VALID_TARGET_PROVIDERS.join(', ')}`,
    );
  }
  return {
    name: '',
    target_type: key,
    connection_params: templates[key] ?? {},
    background: {},
    additional_context: {},
    metadata: {},
  };
}
```

- [ ] **Step 2: Update test to import from src**

Replace the inline function and constant in `tests/unit/cli/redteam-init.spec.ts` with:

```ts
import { describe, expect, it } from 'vitest';
import {
  VALID_TARGET_PROVIDERS,
  buildTargetScaffold,
} from '../../../src/cli/commands/redteam.js';

describe('buildTargetScaffold', () => {
  const mockTemplates: Record<string, unknown> = {
    OPENAI: { api_key: '', model: 'gpt-4', endpoint: 'https://api.openai.com/v1' },
    HUGGING_FACE: { token: '', model_id: '' },
    DATABRICKS: { host: '', token: '' },
    BEDROCK: { region: '', model_id: '' },
    REST: { url: '', method: 'POST' },
    STREAMING: { url: '', method: 'POST' },
    WEBSOCKET: { url: '' },
  };

  it('builds correct scaffold for valid provider', () => {
    const result = buildTargetScaffold('openai', mockTemplates);
    expect(result).toEqual({
      name: '',
      target_type: 'OPENAI',
      connection_params: mockTemplates.OPENAI,
      background: {},
      additional_context: {},
      metadata: {},
    });
  });

  it('handles case-insensitive provider input', () => {
    const result = buildTargetScaffold('Hugging_Face', mockTemplates);
    expect(result.target_type).toBe('HUGGING_FACE');
  });

  it('throws on invalid provider with list of valid providers', () => {
    expect(() => buildTargetScaffold('azure', mockTemplates)).toThrow(
      'Unknown provider "azure". Valid providers: OPENAI, HUGGING_FACE, DATABRICKS, BEDROCK, REST, STREAMING, WEBSOCKET',
    );
  });
});
```

- [ ] **Step 3: Run tests**

Run: `pnpm test -- tests/unit/cli/redteam-init.spec.ts -v`
Expected: 3 tests PASS

- [ ] **Step 4: Run full test suite**

Run: `pnpm test`
Expected: 471 tests pass (468 existing + 3 new)

- [ ] **Step 5: Commit**

```bash
git add src/cli/commands/redteam.ts tests/unit/cli/redteam-init.spec.ts
git commit -m "feat: extract buildTargetScaffold helper for target init"
```

---

### Task 3: Register `targets init` command

**Files:**
- Modify: `src/cli/commands/redteam.ts` (inside `registerTargetCommands`, before the `templates` command at line 940)

- [ ] **Step 1: Add the `init` command**

Insert before the `templates` command (line 940):

```ts
  targets
    .command('init <provider>')
    .description('Scaffold a target config JSON from a provider template')
    .option('--output <file>', 'Output file path')
    .action(async (provider: string, opts) => {
      try {
        renderRedteamHeader();
        const service = await createService();
        const templates = await service.getTargetTemplates();
        const scaffold = buildTargetScaffold(provider, templates);
        const filename = opts.output ?? `${provider.toLowerCase()}-target.json`;
        const outputPath = path.resolve(filename);
        if (fs.existsSync(outputPath)) {
          renderError(
            `File already exists: ${outputPath} (use --output to specify a different path)`,
          );
          process.exit(1);
        }
        fs.writeFileSync(outputPath, JSON.stringify(scaffold, null, 2) + '\n');
        console.log(chalk.bold('\n  Target config scaffolded:\n'));
        console.log(`    File: ${chalk.cyan(outputPath)}`);
        console.log(`    Provider: ${chalk.dim(provider.toUpperCase())}`);
        console.log(
          `\n  ${chalk.yellow('Next steps:')} Edit the file to fill in ${chalk.bold('name')} and credentials, then run:`,
        );
        console.log(
          `    ${chalk.cyan(`airs redteam targets create --config ${filename} --validate`)}\n`,
        );
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });
```

- [ ] **Step 2: Add `path` import if missing**

Check the imports at the top of `redteam.ts`. `fs` is already imported (`import * as fs from 'node:fs'`). Add `path` if not present:

```ts
import * as path from 'node:path';
```

- [ ] **Step 3: Run full test suite**

Run: `pnpm test`
Expected: 471 tests pass

- [ ] **Step 4: Run lint + typecheck**

Run: `pnpm run lint && pnpm tsc --noEmit`
Expected: Clean

- [ ] **Step 5: Commit**

```bash
git add src/cli/commands/redteam.ts
git commit -m "feat: add 'airs redteam targets init' command"
```

---

### Task 4: Manual smoke test + version bump

- [ ] **Step 1: Smoke test — valid provider**

Run: `pnpm run dev redteam targets init openai`
Expected: Creates `openai-target.json` in cwd with scaffold JSON. Prints file path and next-steps hint.

- [ ] **Step 2: Verify scaffold contents**

Check `openai-target.json` has:
```json
{
  "name": "",
  "target_type": "OPENAI",
  "connection_params": { ... template from API ... },
  "background": {},
  "additional_context": {},
  "metadata": {}
}
```

- [ ] **Step 3: Smoke test — file exists error**

Run: `pnpm run dev redteam targets init openai`
Expected: Error — file already exists

- [ ] **Step 4: Smoke test — invalid provider**

Run: `pnpm run dev redteam targets init azure`
Expected: Error listing valid providers

- [ ] **Step 5: Smoke test — custom output**

Run: `pnpm run dev redteam targets init openai --output my-target.json`
Expected: Creates `my-target.json`

- [ ] **Step 6: Clean up test files**

```bash
rm -f openai-target.json my-target.json
```

- [ ] **Step 7: Bump version to 2.3.0**

Update `version` in `package.json` from `"2.2.0"` to `"2.3.0"`.

- [ ] **Step 8: Add changeset**

Create `.changeset/0005-target-init-command.md`:

```md
---
"prisma-airs-cli": minor
---

Add `airs redteam targets init <provider>` command that scaffolds a target config JSON from provider templates (OPENAI, HUGGING_FACE, DATABRICKS, BEDROCK, REST, STREAMING, WEBSOCKET).
```

- [ ] **Step 9: Final commit**

```bash
git add package.json .changeset/0005-target-init-command.md
git commit -m "chore: bump to v2.3.0 + changeset for target init command"
```

---

## Unresolved Questions

None.
