# Profiles Cleanup Command Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `airs runtime profiles cleanup` command that removes duplicate profile revisions, keeping only the latest revision per profile name.

**Architecture:** Pure function `findDuplicateProfiles()` in a dedicated file handles grouping/dedup logic. Renderer functions handle pretty/json output. CLI command wires it all together following the existing `--force`-to-proceed pattern (no interactive prompts). `execSync('git config user.email')` provides the default `--updated-by` value.

**Tech Stack:** TypeScript, Commander, Chalk, Vitest

**Spec:** `docs/superpowers/specs/2026-04-03-profiles-cleanup-command-design.md`

---

### File Map

| Action | File | Responsibility |
|--------|------|----------------|
| Create | `src/cli/commands/profiles-cleanup.ts` | `findDuplicateProfiles()` pure function + `registerCleanupCommand()` CLI wiring |
| Modify | `src/cli/commands/runtime.ts:644` | Import and register cleanup command |
| Modify | `src/cli/renderer/runtime.ts:151` | Add `renderCleanupPreview()` and `renderCleanupResult()` |
| Create | `tests/unit/cli/profiles-cleanup.spec.ts` | Unit tests for `findDuplicateProfiles()` |

---

### Task 1: `findDuplicateProfiles()` — Tests

**Files:**
- Create: `tests/unit/cli/profiles-cleanup.spec.ts`

- [ ] **Step 1: Write the test file**

```typescript
import { describe, expect, it } from 'vitest';
import { findDuplicateProfiles } from '../../../src/cli/commands/profiles-cleanup.js';

describe('findDuplicateProfiles', () => {
  it('returns empty array when no profiles', () => {
    expect(findDuplicateProfiles([])).toEqual([]);
  });

  it('returns empty array when all names are unique', () => {
    const profiles = [
      { profileId: 'a', profileName: 'Alpha', revision: 1 },
      { profileId: 'b', profileName: 'Beta', revision: 1 },
    ];
    expect(findDuplicateProfiles(profiles)).toEqual([]);
  });

  it('keeps highest revision and removes the rest', () => {
    const profiles = [
      { profileId: 'a1', profileName: 'Alpha', revision: 3 },
      { profileId: 'a2', profileName: 'Alpha', revision: 1 },
      { profileId: 'a3', profileName: 'Alpha', revision: 2 },
    ];
    const result = findDuplicateProfiles(profiles);
    expect(result).toHaveLength(1);
    expect(result[0].name).toBe('Alpha');
    expect(result[0].keep).toEqual({ id: 'a1', revision: 3 });
    expect(result[0].remove).toEqual([
      { id: 'a3', revision: 2 },
      { id: 'a2', revision: 1 },
    ]);
  });

  it('handles mix of unique and duplicate profiles', () => {
    const profiles = [
      { profileId: 'a1', profileName: 'Alpha', revision: 2 },
      { profileId: 'a2', profileName: 'Alpha', revision: 1 },
      { profileId: 'b1', profileName: 'Beta', revision: 1 },
    ];
    const result = findDuplicateProfiles(profiles);
    expect(result).toHaveLength(1);
    expect(result[0].name).toBe('Alpha');
  });

  it('treats undefined revision as 0', () => {
    const profiles = [
      { profileId: 'a1', profileName: 'Alpha', revision: 1 },
      { profileId: 'a2', profileName: 'Alpha' },
    ];
    const result = findDuplicateProfiles(profiles);
    expect(result[0].keep).toEqual({ id: 'a1', revision: 1 });
    expect(result[0].remove).toEqual([{ id: 'a2', revision: 0 }]);
  });

  it('sorts remove list by revision descending', () => {
    const profiles = [
      { profileId: 'a1', profileName: 'A', revision: 5 },
      { profileId: 'a2', profileName: 'A', revision: 1 },
      { profileId: 'a3', profileName: 'A', revision: 3 },
      { profileId: 'a4', profileName: 'A', revision: 4 },
      { profileId: 'a5', profileName: 'A', revision: 2 },
    ];
    const result = findDuplicateProfiles(profiles);
    const revisions = result[0].remove.map((r) => r.revision);
    expect(revisions).toEqual([4, 3, 2, 1]);
  });

  it('handles multiple duplicate groups', () => {
    const profiles = [
      { profileId: 'a1', profileName: 'Alpha', revision: 2 },
      { profileId: 'a2', profileName: 'Alpha', revision: 1 },
      { profileId: 'b1', profileName: 'Beta', revision: 3 },
      { profileId: 'b2', profileName: 'Beta', revision: 1 },
      { profileId: 'b3', profileName: 'Beta', revision: 2 },
    ];
    const result = findDuplicateProfiles(profiles);
    expect(result).toHaveLength(2);
    const names = result.map((g) => g.name).sort();
    expect(names).toEqual(['Alpha', 'Beta']);
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `pnpm test -- tests/unit/cli/profiles-cleanup.spec.ts`
Expected: FAIL — module `profiles-cleanup.js` does not exist

- [ ] **Step 3: Commit**

```bash
git add tests/unit/cli/profiles-cleanup.spec.ts
git commit -m "test: add findDuplicateProfiles unit tests"
```

---

### Task 2: `findDuplicateProfiles()` — Implementation

**Files:**
- Create: `src/cli/commands/profiles-cleanup.ts`

- [ ] **Step 1: Write the implementation**

```typescript
import { execSync } from 'node:child_process';
import chalk from 'chalk';
import type { Command } from 'commander';
import { SdkManagementService } from '../../airs/management.js';
import type { SecurityProfileInfo } from '../../airs/types.js';
import { loadConfig } from '../../config/loader.js';
import {
  renderCleanupPreview,
  renderCleanupResult,
  renderError,
  renderRuntimeConfigHeader,
} from '../renderer/index.js';

export interface DuplicateGroup {
  name: string;
  keep: { id: string; revision: number };
  remove: Array<{ id: string; revision: number }>;
}

export interface CleanupDeleteResult {
  id: string;
  revision: number;
  name: string;
  status: 'ok' | 'failed';
  error?: string;
}

export function findDuplicateProfiles(
  profiles: Array<{ profileId: string; profileName: string; revision?: number }>,
): DuplicateGroup[] {
  const groups = new Map<string, Array<{ id: string; revision: number }>>();

  for (const p of profiles) {
    const rev = p.revision ?? 0;
    const entry = { id: p.profileId, revision: rev };
    const existing = groups.get(p.profileName);
    if (existing) {
      existing.push(entry);
    } else {
      groups.set(p.profileName, [entry]);
    }
  }

  const duplicates: DuplicateGroup[] = [];
  for (const [name, entries] of groups) {
    if (entries.length <= 1) continue;
    entries.sort((a, b) => b.revision - a.revision);
    duplicates.push({
      name,
      keep: entries[0],
      remove: entries.slice(1),
    });
  }

  return duplicates;
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `pnpm test -- tests/unit/cli/profiles-cleanup.spec.ts`
Expected: All 7 tests PASS

- [ ] **Step 3: Commit**

```bash
git add src/cli/commands/profiles-cleanup.ts
git commit -m "feat: add findDuplicateProfiles pure function"
```

---

### Task 3: Renderer Functions

**Files:**
- Modify: `src/cli/renderer/runtime.ts:151` — add after `renderProfileDetail()`

- [ ] **Step 1: Add the two renderer functions**

Add after the closing `}` of `renderProfileDetail()` (line 151), before the `renderTopicList` function:

```typescript
/** Render cleanup preview showing duplicate groups. */
export function renderCleanupPreview(
  groups: Array<{
    name: string;
    keep: { id: string; revision: number };
    remove: Array<{ id: string; revision: number }>;
  }>,
  format: 'pretty' | 'json' = 'pretty',
): void {
  if (format === 'json') {
    console.log(
      JSON.stringify(
        {
          duplicates: groups.map((g) => ({
            name: g.name,
            revisions: g.keep.revision,
            keeping: g.keep.revision,
            deleting: g.remove.length,
          })),
          total: groups.reduce((sum, g) => sum + g.remove.length, 0),
        },
        null,
        2,
      ),
    );
    return;
  }

  console.log(chalk.bold('\n  Duplicate Profiles:\n'));

  const nameWidth = Math.max(7, ...groups.map((g) => g.name.length));
  const header = `  ${'Profile'.padEnd(nameWidth)}  Revisions  Keeping   Deleting`;
  console.log(chalk.dim(header));
  console.log(chalk.dim(`  ${'─'.repeat(nameWidth)}  ${'─'.repeat(9)}  ${'─'.repeat(8)}  ${'─'.repeat(8)}`));

  for (const g of groups) {
    const total = g.remove.length + 1;
    console.log(
      `  ${g.name.padEnd(nameWidth)}  ${String(total).padStart(9)}  ${(`rev ${g.keep.revision}`).padStart(8)}  ${String(g.remove.length).padStart(8)}`,
    );
  }

  const totalRemove = groups.reduce((sum, g) => sum + g.remove.length, 0);
  console.log(
    `\n  Total: ${chalk.yellow(String(totalRemove))} old revisions to delete across ${groups.length} profiles\n`,
  );
}

/** Render cleanup deletion results. */
export function renderCleanupResult(
  results: Array<{ id: string; revision: number; name: string; status: 'ok' | 'failed'; error?: string }>,
  format: 'pretty' | 'json' = 'pretty',
): void {
  const deleted = results.filter((r) => r.status === 'ok').length;
  const failed = results.filter((r) => r.status === 'failed').length;

  if (format === 'json') {
    console.log(JSON.stringify({ deleted, failed, details: results }, null, 2));
    return;
  }

  if (failed > 0) {
    console.log(chalk.bold.red('\n  Failures:\n'));
    for (const r of results.filter((r) => r.status === 'failed')) {
      console.log(`    ${chalk.red('✗')} ${r.name} rev ${r.revision}: ${r.error}`);
    }
  }

  const color = failed > 0 ? chalk.yellow : chalk.green;
  console.log(color(`\n  Cleanup complete: ${deleted} deleted, ${failed} failed\n`));
}
```

- [ ] **Step 2: Run lint**

Run: `pnpm run lint`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add src/cli/renderer/runtime.ts
git commit -m "feat: add renderCleanupPreview and renderCleanupResult renderers"
```

---

### Task 4: CLI Command Registration

**Files:**
- Modify: `src/cli/commands/profiles-cleanup.ts` — add `registerCleanupCommand()`
- Modify: `src/cli/commands/runtime.ts:644` — import and register
- Modify: `src/cli/commands/runtime.ts:1-30` — add import

- [ ] **Step 1: Add `registerCleanupCommand()` to `profiles-cleanup.ts`**

Append to the end of `src/cli/commands/profiles-cleanup.ts`:

```typescript
function resolveUpdatedBy(flag?: string): string {
  if (flag) return flag;
  try {
    return execSync('git config user.email', { encoding: 'utf-8' }).trim();
  } catch {
    throw new Error('--updated-by <email> is required (could not detect git user.email)');
  }
}

export function registerCleanupCommand(parent: Command): void {
  parent
    .command('cleanup')
    .description('Delete old profile revisions, keeping only the latest per name')
    .option('--force', 'Skip confirmation — proceed with deletion')
    .option('--updated-by <email>', 'Email for deletion audit (default: git user.email)')
    .option('--output <format>', 'Output format: pretty or json', 'pretty')
    .action(async (opts) => {
      try {
        const fmt = opts.output as 'pretty' | 'json';
        if (fmt === 'pretty') renderRuntimeConfigHeader();

        const config = await loadConfig();
        const service = new SdkManagementService({
          clientId: config.mgmtClientId,
          clientSecret: config.mgmtClientSecret,
          tsgId: config.mgmtTsgId,
          tokenEndpoint: config.mgmtTokenEndpoint,
        });

        const result = await service.listProfiles({ limit: 10000 });
        const groups = findDuplicateProfiles(result.profiles);

        if (groups.length === 0) {
          if (fmt === 'json') {
            console.log(JSON.stringify({ duplicates: [], summary: { deleted: 0, failed: 0 } }));
          } else {
            console.log(chalk.green('\n  No duplicate profiles found.\n'));
          }
          return;
        }

        renderCleanupPreview(groups, fmt);

        if (!opts.force) {
          const totalRemove = groups.reduce((sum, g) => sum + g.remove.length, 0);
          if (fmt === 'pretty') {
            console.log(`  Pass ${chalk.bold('--force')} to delete these revisions.\n`);
          }
          return;
        }

        const updatedBy = resolveUpdatedBy(opts.updatedBy);
        const results: CleanupDeleteResult[] = [];

        for (const group of groups) {
          for (const entry of group.remove) {
            try {
              await service.forceDeleteProfile(entry.id, updatedBy);
              results.push({ ...entry, name: group.name, status: 'ok' });
              if (fmt === 'pretty') {
                console.log(`  ${chalk.green('✓')} ${group.name} rev ${entry.revision}`);
              }
            } catch (err) {
              const msg = err instanceof Error ? err.message : String(err);
              results.push({ ...entry, name: group.name, status: 'failed', error: msg });
              if (fmt === 'pretty') {
                console.log(`  ${chalk.red('✗')} ${group.name} rev ${entry.revision}: ${msg}`);
              }
            }
          }
        }

        renderCleanupResult(results, fmt);

        const failed = results.filter((r) => r.status === 'failed').length;
        if (failed > 0) process.exit(1);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });
}
```

- [ ] **Step 2: Add import in `runtime.ts`**

At the top of `src/cli/commands/runtime.ts`, add to the import block (after line 14):

```typescript
import { registerCleanupCommand } from './profiles-cleanup.js';
```

- [ ] **Step 3: Register cleanup command in `runtime.ts`**

After line 644 (`});` closing the delete command action), before the comment `// Register audit under profiles` (line 646), add:

```typescript
  // Register cleanup under profiles
  registerCleanupCommand(profiles);
```

- [ ] **Step 4: Add renderer imports to `runtime.ts`**

In the import block at lines 16-33, add `renderCleanupPreview` and `renderCleanupResult` to the imports from `'../renderer/index.js'`. These are not needed here since `profiles-cleanup.ts` imports them directly — skip this step.

- [ ] **Step 5: Run type check and lint**

Run: `pnpm tsc --noEmit && pnpm run lint`
Expected: No errors

- [ ] **Step 6: Run all tests**

Run: `pnpm test`
Expected: All tests pass including the new `profiles-cleanup.spec.ts`

- [ ] **Step 7: Commit**

```bash
git add src/cli/commands/profiles-cleanup.ts src/cli/commands/runtime.ts
git commit -m "feat: add 'airs runtime profiles cleanup' command"
```

---

### Task 5: Manual Smoke Test

- [ ] **Step 1: Verify help text**

Run: `pnpm run dev runtime profiles cleanup --help`
Expected: Shows description, `--force`, `--updated-by`, `--output` options

- [ ] **Step 2: Dry run (no --force)**

Run: `pnpm run dev runtime profiles cleanup`
Expected: Shows preview table (or "No duplicate profiles found") and the `--force` hint

- [ ] **Step 3: JSON output**

Run: `pnpm run dev runtime profiles cleanup --output json`
Expected: JSON output of duplicate groups

---

### Task 6: Changeset

**Files:**
- Create: `.changeset/0005-profiles-cleanup.md`

- [ ] **Step 1: Write changeset**

```markdown
---
"@cdot65/prisma-airs-cli": minor
---

Add `airs runtime profiles cleanup` command to remove old profile revisions, keeping only the latest per name. Supports `--force` to skip confirmation, `--updated-by` (defaults to git email), and `--output json`.
```

- [ ] **Step 2: Commit**

```bash
git add .changeset/0005-profiles-cleanup.md
git commit -m "chore: add changeset for profiles cleanup command"
```
