# Autoresearch-Pattern Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the embedded LLM-driven `generate` loop with atomic CLI commands (`create`, `apply`, `eval`, `revert`) that an external agent orchestrates, following the autoresearch pattern.

**Architecture:** Strip the LLM service, memory system, persistence layer, and loop orchestrator. Add four new commands under `airs runtime topics` that operate atomically. `create` handles topic CRUD, `apply` handles profile assignment (additive), `eval` scans a static prompt set, `revert` cleans up. The external agent drives the optimization loop via `program.md`.

**Tech Stack:** TypeScript ESM, Commander.js, `@cdot65/prisma-airs-sdk`, Vitest, Biome

**Decisions:**
- Companion topics for block-intent: agent calls `create` + `apply` twice (once for block, once for allow). CLI stays dumb.
- `report` and `runs` commands: deleted (depend on RunState).
- Audit command: left as-is for now.
- Examples: minimum 2, maximum 5 (AIRS requirement).

---

## File Structure

### New files

| File | Purpose |
|------|---------|
| `src/core/prompt-loader.ts` | Parse CSV prompt sets into TestCase arrays |
| `src/cli/commands/topics-create.ts` | `airs runtime topics create` — create or update a topic |
| `src/cli/commands/topics-apply.ts` | `airs runtime topics apply` — assign topic to profile (additive) |
| `src/cli/commands/topics-eval.ts` | `airs runtime topics eval` — scan static prompts, compute metrics |
| `src/cli/commands/topics-revert.ts` | `airs runtime topics revert` — remove topic from profile, delete |
| `src/cli/renderer/eval.ts` | Terminal + JSON output for eval results |
| `tests/unit/core/prompt-loader.spec.ts` | Tests for CSV parsing and validation |
| `tests/unit/cli/topics-create.spec.ts` | Tests for create command logic |
| `tests/unit/cli/topics-apply.spec.ts` | Tests for apply command logic |
| `tests/unit/cli/topics-eval.spec.ts` | Tests for eval command logic |
| `tests/unit/cli/topics-revert.spec.ts` | Tests for revert command logic |
| `program.md` | Agent instructions for the autoresearch loop |

### Modified files

| File | Change |
|------|--------|
| `src/core/constraints.ts` | Add `MIN_EXAMPLES = 2`, enforce in `validateExamples()` |
| `src/cli/commands/runtime.ts` | Remove generate/resume/report/runs imports; register create/apply/eval/revert |
| `src/cli/renderer/index.ts` | Remove generate re-export; add eval re-export |
| `src/core/types.ts` | Remove loop-specific types (keep CustomTopic, TestCase, TestResult, EfficacyMetrics) |
| `src/index.ts` | Remove llm/memory/persistence/loop exports |
| `package.json` | Remove LangChain deps |
| `tests/helpers/mocks.ts` | Remove loop-specific mocks |
| `tests/unit/core/constraints.spec.ts` | Add test for MIN_EXAMPLES |

### Deleted files/directories

| Path | Reason |
|------|--------|
| `src/llm/` | Entire LLM service layer |
| `src/memory/` | Cross-run learning persistence |
| `src/persistence/` | RunState JSON store |
| `src/core/loop.ts` | Async generator loop |
| `src/cli/commands/generate.ts` | Old generate command |
| `src/cli/commands/resume.ts` | Old resume command |
| `src/cli/commands/report.ts` | Run report viewer (depends on RunState) |
| `src/cli/commands/list.ts` | Run list command (depends on RunState) |
| `src/cli/renderer/generate.ts` | Loop event rendering |
| `tests/unit/llm/` | All LLM tests |
| `tests/unit/memory/` | All memory tests |
| `tests/unit/persistence/` | All persistence tests |
| `tests/unit/core/loop.spec.ts` | Loop tests |
| `tests/integration/loop.integration.spec.ts` | Integration test |
| `tests/unit/report/` | Report tests |

---

### Task 1: Add MIN_EXAMPLES Constraint

**Files:**
- Modify: `src/core/constraints.ts`
- Modify: `tests/unit/core/constraints.spec.ts`

- [ ] **Step 1: Write failing test for minimum examples**

Add to the existing `tests/unit/core/constraints.spec.ts`:

```typescript
it('rejects fewer than MIN_EXAMPLES examples', () => {
  const errors = validateExamples(['Only one example']);
  expect(errors).toContainEqual(
    expect.objectContaining({ field: 'examples', message: expect.stringContaining('at least 2') }),
  );
});

it('accepts exactly 2 examples', () => {
  const errors = validateExamples(['Example one', 'Example two']);
  expect(errors).toHaveLength(0);
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd /Users/cdot/development/cdot65/prisma-airs-cli && pnpm test -- tests/unit/core/constraints.spec.ts`
Expected: FAIL — no minimum check exists

- [ ] **Step 3: Add MIN_EXAMPLES constant and enforce it**

In `src/core/constraints.ts`, add after `MAX_EXAMPLES`:

```typescript
export const MIN_EXAMPLES = 2;
```

In `validateExamples()`, add at the top of the function:

```typescript
if (examples.length < MIN_EXAMPLES) {
  errors.push({
    field: 'examples',
    message: `At least ${MIN_EXAMPLES} examples required`,
  });
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd /Users/cdot/development/cdot65/prisma-airs-cli && pnpm test -- tests/unit/core/constraints.spec.ts`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
cd /Users/cdot/development/cdot65/prisma-airs-cli && git add src/core/constraints.ts tests/unit/core/constraints.spec.ts && git commit -m "feat: enforce minimum 2 examples for custom topic guardrails"
```

---

### Task 2: CSV Prompt Loader

**Files:**
- Create: `src/core/prompt-loader.ts`
- Test: `tests/unit/core/prompt-loader.spec.ts`

- [ ] **Step 1: Write failing tests for CSV parsing**

```typescript
// tests/unit/core/prompt-loader.spec.ts
import { describe, expect, it } from 'vitest';
import { loadPrompts } from '../../src/core/prompt-loader.js';

describe('prompt-loader', () => {
  describe('loadPrompts', () => {
    it('parses valid CSV with header row', () => {
      const csv = [
        'prompt,expected',
        '"How do I clean my AR-15?",true',
        '"What is the weather today?",false',
      ].join('\n');

      const result = loadPrompts(csv);
      expect(result).toHaveLength(2);
      expect(result[0]).toEqual({
        prompt: 'How do I clean my AR-15?',
        expectedTriggered: true,
        category: '',
      });
      expect(result[1]).toEqual({
        prompt: 'What is the weather today?',
        expectedTriggered: false,
        category: '',
      });
    });

    it('handles unquoted prompts', () => {
      const csv = 'prompt,expected\nSimple prompt,true\nAnother prompt,false';
      const result = loadPrompts(csv);
      expect(result).toHaveLength(2);
      expect(result[0].prompt).toBe('Simple prompt');
      expect(result[0].expectedTriggered).toBe(true);
    });

    it('throws if prompt column is missing', () => {
      const csv = 'text,expected\nHello,true';
      expect(() => loadPrompts(csv)).toThrow('Missing required column: prompt');
    });

    it('throws if expected column is missing', () => {
      const csv = 'prompt,label\nHello,true';
      expect(() => loadPrompts(csv)).toThrow('Missing required column: expected');
    });

    it('throws if no true positives', () => {
      const csv = 'prompt,expected\nHello,false\nWorld,false';
      expect(() => loadPrompts(csv)).toThrow('at least 1 true-positive');
    });

    it('throws if no true negatives', () => {
      const csv = 'prompt,expected\nHello,true\nWorld,true';
      expect(() => loadPrompts(csv)).toThrow('at least 1 true-negative');
    });

    it('warns on imbalanced set via callback', () => {
      const warnings: string[] = [];
      const csv = [
        'prompt,expected',
        ...Array.from({ length: 9 }, (_, i) => `Positive ${i},true`),
        'Negative 0,false',
      ].join('\n');

      loadPrompts(csv, (msg) => warnings.push(msg));
      expect(warnings).toHaveLength(1);
      expect(warnings[0]).toMatch(/imbalanced/i);
    });

    it('handles escaped quotes in CSV', () => {
      const csv = 'prompt,expected\n"He said ""hello"" to me",false\nother,true';
      const result = loadPrompts(csv);
      expect(result[0].prompt).toBe('He said "hello" to me');
    });
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd /Users/cdot/development/cdot65/prisma-airs-cli && pnpm test -- tests/unit/core/prompt-loader.spec.ts`
Expected: FAIL — module not found

- [ ] **Step 3: Implement prompt loader**

```typescript
// src/core/prompt-loader.ts
import type { TestCase } from './types.js';

/**
 * Parse a CSV string of test prompts into TestCase objects.
 * Expects columns: prompt, expected (boolean as "true"/"false").
 */
export function loadPrompts(
  csv: string,
  onWarning?: (message: string) => void,
): TestCase[] {
  const lines = csv.trim().split('\n');
  if (lines.length < 2) {
    throw new Error('CSV must have a header row and at least one data row');
  }

  const header = parseCsvLine(lines[0]).map((h) => h.trim().toLowerCase());
  const promptIdx = header.indexOf('prompt');
  const expectedIdx = header.indexOf('expected');

  if (promptIdx === -1) throw new Error('Missing required column: prompt');
  if (expectedIdx === -1) throw new Error('Missing required column: expected');

  const cases: TestCase[] = [];

  for (let i = 1; i < lines.length; i++) {
    const line = lines[i].trim();
    if (!line) continue;

    const fields = parseCsvLine(line);
    const prompt = fields[promptIdx]?.trim() ?? '';
    const expected = fields[expectedIdx]?.trim().toLowerCase() === 'true';

    if (!prompt) continue;

    cases.push({ prompt, expectedTriggered: expected, category: '' });
  }

  const positives = cases.filter((c) => c.expectedTriggered).length;
  const negatives = cases.filter((c) => !c.expectedTriggered).length;

  if (positives === 0) throw new Error('Prompt set must contain at least 1 true-positive');
  if (negatives === 0) throw new Error('Prompt set must contain at least 1 true-negative');

  const ratio = Math.max(positives, negatives) / cases.length;
  if (ratio > 0.8 && onWarning) {
    onWarning(
      `Imbalanced prompt set: ${positives} positives, ${negatives} negatives (${(ratio * 100).toFixed(0)}% one class)`,
    );
  }

  return cases;
}

/** Minimal RFC 4180 CSV line parser supporting quoted fields with escaped quotes. */
function parseCsvLine(line: string): string[] {
  const fields: string[] = [];
  let current = '';
  let inQuotes = false;

  for (let i = 0; i < line.length; i++) {
    const ch = line[i];

    if (inQuotes) {
      if (ch === '"') {
        if (i + 1 < line.length && line[i + 1] === '"') {
          current += '"';
          i++;
        } else {
          inQuotes = false;
        }
      } else {
        current += ch;
      }
    } else if (ch === '"') {
      inQuotes = true;
    } else if (ch === ',') {
      fields.push(current);
      current = '';
    } else {
      current += ch;
    }
  }

  fields.push(current);
  return fields;
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd /Users/cdot/development/cdot65/prisma-airs-cli && pnpm test -- tests/unit/core/prompt-loader.spec.ts`
Expected: All 8 tests PASS

- [ ] **Step 5: Commit**

```bash
cd /Users/cdot/development/cdot65/prisma-airs-cli && git add src/core/prompt-loader.ts tests/unit/core/prompt-loader.spec.ts && git commit -m "feat: add CSV prompt loader for static test sets"
```

---

### Task 3: Eval Renderer

**Files:**
- Create: `src/cli/renderer/eval.ts`
- Modify: `src/cli/renderer/index.ts`

- [ ] **Step 1: Write eval renderer**

```typescript
// src/cli/renderer/eval.ts
import chalk from 'chalk';
import type { EfficacyMetrics, TestResult } from '../../core/types.js';

export interface EvalOutput {
  profile: string;
  topic: string;
  metrics: {
    tp: number;
    tn: number;
    fp: number;
    fn: number;
    tpr: number;
    tnr: number;
    coverage: number;
    f1: number;
    total: number;
  };
  false_positives: Array<{ prompt: string; expected: boolean; actual: boolean }>;
  false_negatives: Array<{ prompt: string; expected: boolean; actual: boolean }>;
}

export function buildEvalOutput(
  profile: string,
  topic: string,
  metrics: EfficacyMetrics,
  results: TestResult[],
): EvalOutput {
  const fps = results
    .filter((r) => !r.testCase.expectedTriggered && r.actualTriggered)
    .map((r) => ({ prompt: r.testCase.prompt, expected: false, actual: true }));

  const fns = results
    .filter((r) => r.testCase.expectedTriggered && !r.actualTriggered)
    .map((r) => ({ prompt: r.testCase.prompt, expected: true, actual: false }));

  return {
    profile,
    topic,
    metrics: {
      tp: metrics.truePositives,
      tn: metrics.trueNegatives,
      fp: metrics.falsePositives,
      fn: metrics.falseNegatives,
      tpr: metrics.truePositiveRate,
      tnr: metrics.trueNegativeRate,
      coverage: metrics.coverage,
      f1: metrics.f1Score,
      total: results.length,
    },
    false_positives: fps,
    false_negatives: fns,
  };
}

export function renderEvalTerminal(output: EvalOutput): void {
  const coverageColor =
    output.metrics.coverage >= 0.9
      ? chalk.green
      : output.metrics.coverage >= 0.7
        ? chalk.yellow
        : chalk.red;

  console.log(chalk.bold('\n  Eval Results'));
  console.log(chalk.dim('  ─────────────────────────'));
  console.log(`  Profile: ${chalk.white(output.profile)}`);
  console.log(`  Topic:   ${chalk.white(output.topic)}`);

  console.log(chalk.bold('\n  Metrics:'));
  console.log(`    Coverage:  ${coverageColor(`${(output.metrics.coverage * 100).toFixed(1)}%`)}`);
  console.log(`    TPR:       ${(output.metrics.tpr * 100).toFixed(1)}%`);
  console.log(`    TNR:       ${(output.metrics.tnr * 100).toFixed(1)}%`);
  console.log(`    F1:        ${output.metrics.f1.toFixed(3)}`);
  console.log(
    chalk.dim(
      `    TP: ${output.metrics.tp}  TN: ${output.metrics.tn}  FP: ${output.metrics.fp}  FN: ${output.metrics.fn}  Total: ${output.metrics.total}`,
    ),
  );

  if (output.false_positives.length > 0) {
    console.log(chalk.bold.yellow('\n  False Positives:'));
    for (const fp of output.false_positives) {
      console.log(`    ${chalk.yellow('●')} ${fp.prompt}`);
    }
  }

  if (output.false_negatives.length > 0) {
    console.log(chalk.bold.red('\n  False Negatives:'));
    for (const fn of output.false_negatives) {
      console.log(`    ${chalk.red('●')} ${fn.prompt}`);
    }
  }

  console.log();
}
```

- [ ] **Step 2: Add eval export to renderer index**

In `src/cli/renderer/index.ts`, add:

```typescript
export * from './eval.js';
```

- [ ] **Step 3: Commit**

```bash
cd /Users/cdot/development/cdot65/prisma-airs-cli && git add src/cli/renderer/eval.ts src/cli/renderer/index.ts && git commit -m "feat: add eval renderer for terminal and JSON output"
```

---

### Task 4: `topics create` Command

**Files:**
- Create: `src/cli/commands/topics-create.ts`
- Test: `tests/unit/cli/topics-create.spec.ts`

- [ ] **Step 1: Write failing tests**

```typescript
// tests/unit/cli/topics-create.spec.ts
import { describe, expect, it, vi } from 'vitest';
import { createOrUpdateTopic } from '../../src/cli/commands/topics-create.js';
import { createMockManagementService } from '../helpers/mocks.js';

describe('topics-create', () => {
  describe('createOrUpdateTopic', () => {
    it('creates a new topic when none exists with that name', async () => {
      const mgmt = createMockManagementService();
      mgmt.listTopics = vi.fn().mockResolvedValue([]);

      const result = await createOrUpdateTopic(mgmt, {
        name: 'Test Topic',
        description: 'A test description',
        examples: ['Example 1', 'Example 2'],
      });

      expect(result.topicId).toBe('topic-1');
      expect(result.topicName).toBe('Test Topic');
      expect(result.created).toBe(true);
    });

    it('updates existing topic when name matches', async () => {
      const mgmt = createMockManagementService();
      mgmt.listTopics = vi.fn().mockResolvedValue([
        { topic_id: 'existing-1', topic_name: 'Test Topic', description: 'old', examples: [] },
      ]);
      mgmt.updateTopic = vi.fn().mockResolvedValue({
        topic_id: 'existing-1',
        topic_name: 'Test Topic',
        description: 'A test description',
        examples: ['Example 1', 'Example 2'],
        revision: 2,
      });

      const result = await createOrUpdateTopic(mgmt, {
        name: 'Test Topic',
        description: 'A test description',
        examples: ['Example 1', 'Example 2'],
      });

      expect(result.topicId).toBe('existing-1');
      expect(result.created).toBe(false);
      expect(mgmt.updateTopic).toHaveBeenCalledWith('existing-1', expect.any(Object));
    });

    it('throws on constraint validation failure — name too long', async () => {
      const mgmt = createMockManagementService();
      mgmt.listTopics = vi.fn().mockResolvedValue([]);

      await expect(
        createOrUpdateTopic(mgmt, {
          name: 'x'.repeat(200),
          description: 'A test description',
          examples: ['Example 1', 'Example 2'],
        }),
      ).rejects.toThrow(/bytes/);
    });

    it('throws on constraint validation failure — fewer than 2 examples', async () => {
      const mgmt = createMockManagementService();
      mgmt.listTopics = vi.fn().mockResolvedValue([]);

      await expect(
        createOrUpdateTopic(mgmt, {
          name: 'Test Topic',
          description: 'A test description',
          examples: ['Only one'],
        }),
      ).rejects.toThrow(/at least 2/);
    });

    it('throws on constraint validation failure — more than 5 examples', async () => {
      const mgmt = createMockManagementService();
      mgmt.listTopics = vi.fn().mockResolvedValue([]);

      await expect(
        createOrUpdateTopic(mgmt, {
          name: 'Test Topic',
          description: 'desc',
          examples: ['a', 'b', 'c', 'd', 'e', 'f'],
        }),
      ).rejects.toThrow(/5/);
    });
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd /Users/cdot/development/cdot65/prisma-airs-cli && pnpm test -- tests/unit/cli/topics-create.spec.ts`
Expected: FAIL — module not found

- [ ] **Step 3: Implement create command**

```typescript
// src/cli/commands/topics-create.ts
import type { Command } from 'commander';
import { SdkManagementService } from '../../airs/management.js';
import type { ManagementService } from '../../airs/types.js';
import { loadConfig } from '../../config/loader.js';
import { validateTopic } from '../../core/constraints.js';
import { renderError } from '../renderer/index.js';

export interface CreateInput {
  name: string;
  description: string;
  examples: string[];
}

export interface CreateOutput {
  topicId: string;
  topicName: string;
  revision: number;
  created: boolean;
}

/**
 * Create a new custom topic, or update it if one with the same name exists.
 * Validates AIRS constraints (name, description, examples, combined length).
 */
export async function createOrUpdateTopic(
  mgmt: ManagementService,
  input: CreateInput,
): Promise<CreateOutput> {
  const topic = { name: input.name, description: input.description, examples: input.examples };
  const errors = validateTopic(topic);
  if (errors.length > 0) {
    throw new Error(errors.map((e) => `${e.field}: ${e.message}`).join('; '));
  }

  const existing = await mgmt.listTopics();
  const match = existing.find((t) => t.topic_name === input.name);

  const request = {
    topic_name: input.name,
    description: input.description,
    examples: input.examples,
  };

  const result = match
    ? await mgmt.updateTopic(match.topic_id, request)
    : await mgmt.createTopic(request);

  return {
    topicId: result.topic_id,
    topicName: result.topic_name,
    revision: result.revision ?? 0,
    created: !match,
  };
}

export function registerCreateCommand(parent: Command): void {
  parent
    .command('create')
    .description('Create or update a custom topic definition')
    .requiredOption('--name <name>', 'Topic name')
    .requiredOption('--description <desc>', 'Topic description')
    .requiredOption('--examples <examples...>', 'Example prompts (2-5 required)')
    .option('--format <format>', 'Output format: json or terminal', 'terminal')
    .action(async (opts) => {
      try {
        const config = await loadConfig();
        const mgmt = new SdkManagementService({
          clientId: config.mgmtClientId,
          clientSecret: config.mgmtClientSecret,
          tsgId: config.mgmtTsgId,
          tokenEndpoint: config.mgmtTokenEndpoint,
        });

        const result = await createOrUpdateTopic(mgmt, {
          name: opts.name,
          description: opts.description,
          examples: opts.examples,
        });

        if (opts.format === 'json') {
          console.log(JSON.stringify(result, null, 2));
        } else {
          console.log(`\n  Topic ${result.created ? 'created' : 'updated'}: ${result.topicName}`);
          console.log(`  ID:       ${result.topicId}`);
          console.log(`  Revision: ${result.revision}\n`);
        }
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd /Users/cdot/development/cdot65/prisma-airs-cli && pnpm test -- tests/unit/cli/topics-create.spec.ts`
Expected: All 5 tests PASS

- [ ] **Step 5: Commit**

```bash
cd /Users/cdot/development/cdot65/prisma-airs-cli && git add src/cli/commands/topics-create.ts tests/unit/cli/topics-create.spec.ts && git commit -m "feat: add topics create command for topic CRUD"
```

---

### Task 5: `topics apply` Command (Additive Profile Assignment)

**Files:**
- Create: `src/cli/commands/topics-apply.ts`
- Test: `tests/unit/cli/topics-apply.spec.ts`

- [ ] **Step 1: Write failing tests**

```typescript
// tests/unit/cli/topics-apply.spec.ts
import { describe, expect, it, vi } from 'vitest';
import { applyTopicToProfile } from '../../src/cli/commands/topics-apply.js';
import { createMockManagementService } from '../helpers/mocks.js';

describe('topics-apply', () => {
  describe('applyTopicToProfile', () => {
    it('assigns a topic to a profile with block intent', async () => {
      const mgmt = createMockManagementService();
      mgmt.listTopics = vi.fn().mockResolvedValue([
        { topic_id: 'topic-1', topic_name: 'My Topic', revision: 1 },
      ]);
      mgmt.getProfileTopics = vi.fn().mockResolvedValue([]);
      mgmt.assignTopicsToProfile = vi.fn().mockResolvedValue(undefined);

      const result = await applyTopicToProfile(mgmt, {
        profileName: 'test-profile',
        topicName: 'My Topic',
        intent: 'block',
      });

      expect(result.topicName).toBe('My Topic');
      expect(result.intent).toBe('block');
      expect(mgmt.assignTopicsToProfile).toHaveBeenCalledWith(
        'test-profile',
        [{ topicId: 'topic-1', topicName: 'My Topic', action: 'block' }],
        'allow',
      );
    });

    it('assigns a topic with allow intent', async () => {
      const mgmt = createMockManagementService();
      mgmt.listTopics = vi.fn().mockResolvedValue([
        { topic_id: 'topic-1', topic_name: 'My Topic', revision: 1 },
      ]);
      mgmt.getProfileTopics = vi.fn().mockResolvedValue([]);
      mgmt.assignTopicsToProfile = vi.fn().mockResolvedValue(undefined);

      await applyTopicToProfile(mgmt, {
        profileName: 'test-profile',
        topicName: 'My Topic',
        intent: 'allow',
      });

      expect(mgmt.assignTopicsToProfile).toHaveBeenCalledWith(
        'test-profile',
        [{ topicId: 'topic-1', topicName: 'My Topic', action: 'allow' }],
        'block',
      );
    });

    it('preserves existing topics when adding a new one (additive)', async () => {
      const mgmt = createMockManagementService();
      mgmt.listTopics = vi.fn().mockResolvedValue([
        { topic_id: 'topic-1', topic_name: 'Existing Topic', revision: 1 },
        { topic_id: 'topic-2', topic_name: 'New Topic', revision: 1 },
      ]);
      mgmt.getProfileTopics = vi.fn().mockResolvedValue([
        { topicId: 'topic-1', topicName: 'Existing Topic', action: 'block' },
      ]);
      mgmt.assignTopicsToProfile = vi.fn().mockResolvedValue(undefined);

      await applyTopicToProfile(mgmt, {
        profileName: 'test-profile',
        topicName: 'New Topic',
        intent: 'block',
      });

      const call = (mgmt.assignTopicsToProfile as ReturnType<typeof vi.fn>).mock.calls[0];
      const topics = call[1] as Array<{ topicName: string }>;
      expect(topics).toHaveLength(2);
      expect(topics.map((t) => t.topicName)).toContain('Existing Topic');
      expect(topics.map((t) => t.topicName)).toContain('New Topic');
    });

    it('replaces existing topic entry when re-applying same name', async () => {
      const mgmt = createMockManagementService();
      mgmt.listTopics = vi.fn().mockResolvedValue([
        { topic_id: 'topic-1', topic_name: 'My Topic', revision: 2 },
      ]);
      mgmt.getProfileTopics = vi.fn().mockResolvedValue([
        { topicId: 'topic-1', topicName: 'My Topic', action: 'block' },
      ]);
      mgmt.assignTopicsToProfile = vi.fn().mockResolvedValue(undefined);

      await applyTopicToProfile(mgmt, {
        profileName: 'test-profile',
        topicName: 'My Topic',
        intent: 'block',
      });

      const call = (mgmt.assignTopicsToProfile as ReturnType<typeof vi.fn>).mock.calls[0];
      const topics = call[1] as Array<{ topicName: string }>;
      expect(topics).toHaveLength(1);
    });

    it('throws when topic not found', async () => {
      const mgmt = createMockManagementService();
      mgmt.listTopics = vi.fn().mockResolvedValue([]);

      await expect(
        applyTopicToProfile(mgmt, {
          profileName: 'test-profile',
          topicName: 'Missing',
          intent: 'block',
        }),
      ).rejects.toThrow(/not found/);
    });
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd /Users/cdot/development/cdot65/prisma-airs-cli && pnpm test -- tests/unit/cli/topics-apply.spec.ts`
Expected: FAIL — module not found

- [ ] **Step 3: Implement apply command**

```typescript
// src/cli/commands/topics-apply.ts
import type { Command } from 'commander';
import { SdkManagementService } from '../../airs/management.js';
import type { ManagementService } from '../../airs/types.js';
import { loadConfig } from '../../config/loader.js';
import { renderError } from '../renderer/index.js';

export interface ApplyInput {
  profileName: string;
  topicName: string;
  intent: 'allow' | 'block';
}

export interface ApplyOutput {
  topicId: string;
  topicName: string;
  profileName: string;
  intent: string;
}

/**
 * Assign a topic to a security profile. Additive — preserves existing
 * topics already on the profile. If the same topic name is already assigned,
 * its entry is replaced (picks up new revision).
 */
export async function applyTopicToProfile(
  mgmt: ManagementService,
  input: ApplyInput,
): Promise<ApplyOutput> {
  // Find topic by name
  const allTopics = await mgmt.listTopics();
  const match = allTopics.find((t) => t.topic_name === input.topicName);
  if (!match) {
    throw new Error(`Topic "${input.topicName}" not found. Create it first with "topics create".`);
  }

  // Read existing profile topics to preserve them
  const existing = await mgmt.getProfileTopics(input.profileName);

  // Build merged topic list: replace if same name, otherwise keep existing
  const merged = existing
    .filter((t) => t.topicName !== input.topicName)
    .map((t) => ({ topicId: t.topicId, topicName: t.topicName, action: t.action }));

  merged.push({
    topicId: match.topic_id,
    topicName: match.topic_name,
    action: input.intent,
  });

  // Guardrail-level action: 'allow' for block-intent, 'block' for allow-intent
  const guardrailAction = input.intent === 'block' ? 'allow' : 'block';

  await mgmt.assignTopicsToProfile(input.profileName, merged, guardrailAction);

  return {
    topicId: match.topic_id,
    topicName: match.topic_name,
    profileName: input.profileName,
    intent: input.intent,
  };
}

export function registerApplyCommand(parent: Command): void {
  parent
    .command('apply')
    .description('Assign a topic to a security profile (additive)')
    .requiredOption('--profile <name>', 'Security profile name')
    .requiredOption('--name <name>', 'Topic name to assign')
    .option('--intent <intent>', 'Topic intent: block or allow', 'block')
    .option('--format <format>', 'Output format: json or terminal', 'terminal')
    .action(async (opts) => {
      try {
        const config = await loadConfig();
        const mgmt = new SdkManagementService({
          clientId: config.mgmtClientId,
          clientSecret: config.mgmtClientSecret,
          tsgId: config.mgmtTsgId,
          tokenEndpoint: config.mgmtTokenEndpoint,
        });

        const result = await applyTopicToProfile(mgmt, {
          profileName: opts.profile,
          topicName: opts.name,
          intent: opts.intent as 'allow' | 'block',
        });

        if (opts.format === 'json') {
          console.log(JSON.stringify(result, null, 2));
        } else {
          console.log(`\n  Applied: ${result.topicName}`);
          console.log(`  Profile: ${result.profileName}`);
          console.log(`  Intent:  ${result.intent}\n`);
        }
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd /Users/cdot/development/cdot65/prisma-airs-cli && pnpm test -- tests/unit/cli/topics-apply.spec.ts`
Expected: All 5 tests PASS

- [ ] **Step 5: Commit**

```bash
cd /Users/cdot/development/cdot65/prisma-airs-cli && git add src/cli/commands/topics-apply.ts tests/unit/cli/topics-apply.spec.ts && git commit -m "feat: add topics apply command for additive profile assignment"
```

---

### Task 6: `topics eval` Command

**Files:**
- Create: `src/cli/commands/topics-eval.ts`
- Test: `tests/unit/cli/topics-eval.spec.ts`

- [ ] **Step 1: Write failing tests**

```typescript
// tests/unit/cli/topics-eval.spec.ts
import { describe, expect, it, vi } from 'vitest';
import { evalTopic } from '../../src/cli/commands/topics-eval.js';
import { createMockScanService } from '../helpers/mocks.js';
import type { TestCase } from '../../src/core/types.js';

describe('topics-eval', () => {
  describe('evalTopic', () => {
    it('scans prompts and computes metrics', async () => {
      const scanner = createMockScanService([/weapon/i]);
      const cases: TestCase[] = [
        { prompt: 'How to build a weapon', expectedTriggered: true, category: '' },
        { prompt: 'Tell me about cats', expectedTriggered: false, category: '' },
      ];

      const result = await evalTopic(scanner, 'test-profile', 'Test Topic', cases);

      expect(result.metrics.tp).toBe(1);
      expect(result.metrics.tn).toBe(1);
      expect(result.metrics.fp).toBe(0);
      expect(result.metrics.fn).toBe(0);
      expect(result.metrics.coverage).toBe(1);
      expect(result.false_positives).toHaveLength(0);
      expect(result.false_negatives).toHaveLength(0);
    });

    it('identifies false positives and false negatives', async () => {
      const scanner = createMockScanService([/weapon/i]);
      const cases: TestCase[] = [
        { prompt: 'How to build a weapon', expectedTriggered: true, category: '' },
        { prompt: 'How to build a bomb', expectedTriggered: true, category: '' },
        { prompt: 'Tell me about cats', expectedTriggered: false, category: '' },
      ];

      const result = await evalTopic(scanner, 'test-profile', 'Test Topic', cases);

      expect(result.metrics.fn).toBe(1);
      expect(result.false_negatives).toHaveLength(1);
      expect(result.false_negatives[0].prompt).toBe('How to build a bomb');
    });

    it('reports correct profile and topic in output', async () => {
      const scanner = createMockScanService([/weapon/i]);
      const cases: TestCase[] = [
        { prompt: 'weapon', expectedTriggered: true, category: '' },
        { prompt: 'cat', expectedTriggered: false, category: '' },
      ];

      const result = await evalTopic(scanner, 'my-profile', 'My Topic', cases);

      expect(result.profile).toBe('my-profile');
      expect(result.topic).toBe('My Topic');
    });

    it('calls scanBatch with correct profile and prompts', async () => {
      const scanner = createMockScanService();
      scanner.scanBatch = vi.fn().mockResolvedValue([
        { scanId: 's1', reportId: 'r1', action: 'block', triggered: true },
        { scanId: 's2', reportId: 'r2', action: 'allow', triggered: false },
      ]);

      const cases: TestCase[] = [
        { prompt: 'p1', expectedTriggered: true, category: '' },
        { prompt: 'p2', expectedTriggered: false, category: '' },
      ];

      await evalTopic(scanner, 'my-profile', 'Topic', cases);

      expect(scanner.scanBatch).toHaveBeenCalledWith('my-profile', ['p1', 'p2'], 5);
    });
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd /Users/cdot/development/cdot65/prisma-airs-cli && pnpm test -- tests/unit/cli/topics-eval.spec.ts`
Expected: FAIL — module not found

- [ ] **Step 3: Implement eval command**

```typescript
// src/cli/commands/topics-eval.ts
import { readFile } from 'node:fs/promises';
import type { Command } from 'commander';
import { AirsScanService, RateLimitedScanService } from '../../airs/scanner.js';
import type { ScanService } from '../../airs/types.js';
import { loadConfig } from '../../config/loader.js';
import { computeMetrics } from '../../core/metrics.js';
import { loadPrompts } from '../../core/prompt-loader.js';
import type { TestCase, TestResult } from '../../core/types.js';
import {
  type EvalOutput,
  buildEvalOutput,
  renderEvalTerminal,
  renderError,
} from '../renderer/index.js';

/**
 * Scan a static prompt set against a profile and compute evaluation metrics.
 * Returns structured output with FP/FN details for agent reasoning.
 */
export async function evalTopic(
  scanner: ScanService,
  profileName: string,
  topicName: string,
  cases: TestCase[],
  concurrency = 5,
): Promise<EvalOutput> {
  const prompts = cases.map((c) => c.prompt);
  const scanResults = await scanner.scanBatch(profileName, prompts, concurrency);

  const testResults: TestResult[] = cases.map((testCase, i) => {
    const scan = scanResults[i];
    return {
      testCase,
      actualTriggered: scan.triggered,
      scanAction: scan.action,
      scanId: scan.scanId,
      reportId: scan.reportId,
      correct: testCase.expectedTriggered === scan.triggered,
    };
  });

  const metrics = computeMetrics(testResults);
  return buildEvalOutput(profileName, topicName, metrics, testResults);
}

export function registerEvalCommand(parent: Command): void {
  parent
    .command('eval')
    .description('Evaluate a topic against a static prompt set and compute metrics')
    .requiredOption('--profile <name>', 'Security profile name')
    .requiredOption('--prompts <path>', 'Path to CSV file with prompt,expected columns')
    .option('--topic <name>', 'Topic name (for output labeling)', 'unknown')
    .option('--format <format>', 'Output format: json or terminal', 'terminal')
    .option('--rate <n>', 'Max AIRS scan API calls per second')
    .option('--concurrency <n>', 'Concurrent scan requests', '5')
    .action(async (opts) => {
      try {
        const config = await loadConfig();
        const csvContent = await readFile(opts.prompts, 'utf-8');
        const cases = loadPrompts(csvContent, (msg) => console.warn(`  Warning: ${msg}`));

        let scanner: ScanService = new AirsScanService(config.airsApiKey);
        if (opts.rate) {
          scanner = new RateLimitedScanService(scanner, Number.parseInt(opts.rate, 10));
        }

        const concurrency = Number.parseInt(opts.concurrency, 10);
        const result = await evalTopic(scanner, opts.profile, opts.topic, cases, concurrency);

        if (opts.format === 'json') {
          console.log(JSON.stringify(result, null, 2));
        } else {
          renderEvalTerminal(result);
        }
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd /Users/cdot/development/cdot65/prisma-airs-cli && pnpm test -- tests/unit/cli/topics-eval.spec.ts`
Expected: All 4 tests PASS

- [ ] **Step 5: Commit**

```bash
cd /Users/cdot/development/cdot65/prisma-airs-cli && git add src/cli/commands/topics-eval.ts tests/unit/cli/topics-eval.spec.ts && git commit -m "feat: add topics eval command for static prompt set evaluation"
```

---

### Task 7: `topics revert` Command

**Files:**
- Create: `src/cli/commands/topics-revert.ts`
- Test: `tests/unit/cli/topics-revert.spec.ts`

- [ ] **Step 1: Write failing tests**

```typescript
// tests/unit/cli/topics-revert.spec.ts
import { describe, expect, it, vi } from 'vitest';
import { revertTopic } from '../../src/cli/commands/topics-revert.js';
import { createMockManagementService } from '../helpers/mocks.js';

describe('topics-revert', () => {
  describe('revertTopic', () => {
    it('removes topic from profile and deletes it', async () => {
      const mgmt = createMockManagementService();
      mgmt.listTopics = vi.fn().mockResolvedValue([
        { topic_id: 'topic-1', topic_name: 'My Topic' },
      ]);
      mgmt.getProfileTopics = vi.fn().mockResolvedValue([
        { topicId: 'topic-1', topicName: 'My Topic', action: 'block' },
      ]);
      mgmt.assignTopicsToProfile = vi.fn().mockResolvedValue(undefined);
      mgmt.forceDeleteTopic = vi.fn().mockResolvedValue({ message: 'deleted' });

      const result = await revertTopic(mgmt, 'test-profile', 'My Topic');

      expect(result.deleted).toEqual(['topic-1']);
      expect(mgmt.forceDeleteTopic).toHaveBeenCalledWith('topic-1', undefined);
    });

    it('preserves other topics on the profile', async () => {
      const mgmt = createMockManagementService();
      mgmt.listTopics = vi.fn().mockResolvedValue([
        { topic_id: 'topic-1', topic_name: 'Remove Me' },
        { topic_id: 'topic-2', topic_name: 'Keep Me' },
      ]);
      mgmt.getProfileTopics = vi.fn().mockResolvedValue([
        { topicId: 'topic-1', topicName: 'Remove Me', action: 'block' },
        { topicId: 'topic-2', topicName: 'Keep Me', action: 'allow' },
      ]);
      mgmt.assignTopicsToProfile = vi.fn().mockResolvedValue(undefined);
      mgmt.forceDeleteTopic = vi.fn().mockResolvedValue({ message: 'deleted' });

      await revertTopic(mgmt, 'test-profile', 'Remove Me');

      const call = (mgmt.assignTopicsToProfile as ReturnType<typeof vi.fn>).mock.calls[0];
      const topics = call[1] as Array<{ topicName: string }>;
      expect(topics).toHaveLength(1);
      expect(topics[0].topicName).toBe('Keep Me');
    });

    it('throws when topic not found', async () => {
      const mgmt = createMockManagementService();
      mgmt.listTopics = vi.fn().mockResolvedValue([]);

      await expect(revertTopic(mgmt, 'test-profile', 'Missing')).rejects.toThrow(/not found/);
    });
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd /Users/cdot/development/cdot65/prisma-airs-cli && pnpm test -- tests/unit/cli/topics-revert.spec.ts`
Expected: FAIL — module not found

- [ ] **Step 3: Implement revert command**

```typescript
// src/cli/commands/topics-revert.ts
import type { Command } from 'commander';
import { SdkManagementService } from '../../airs/management.js';
import type { ManagementService } from '../../airs/types.js';
import { loadConfig } from '../../config/loader.js';
import { renderError } from '../renderer/index.js';

export interface RevertOutput {
  profileName: string;
  deleted: string[];
}

/**
 * Remove a topic from a profile and delete it from AIRS.
 * Preserves other topics already assigned to the profile.
 */
export async function revertTopic(
  mgmt: ManagementService,
  profileName: string,
  topicName: string,
): Promise<RevertOutput> {
  const topics = await mgmt.listTopics();
  const match = topics.find((t) => t.topic_name === topicName);
  if (!match) {
    throw new Error(`Topic "${topicName}" not found`);
  }

  // Read current profile topics and remove the target
  const profileTopics = await mgmt.getProfileTopics(profileName);
  const remaining = profileTopics
    .filter((t) => t.topicName !== topicName)
    .map((t) => ({ topicId: t.topicId, topicName: t.topicName, action: t.action }));

  await mgmt.assignTopicsToProfile(profileName, remaining);

  // Force-delete the topic
  await mgmt.forceDeleteTopic(match.topic_id);

  return { profileName, deleted: [match.topic_id] };
}

export function registerRevertCommand(parent: Command): void {
  parent
    .command('revert')
    .description('Remove a custom topic from a profile and delete it')
    .requiredOption('--profile <name>', 'Security profile name')
    .requiredOption('--name <name>', 'Topic name to remove')
    .option('--format <format>', 'Output format: json or terminal', 'terminal')
    .action(async (opts) => {
      try {
        const config = await loadConfig();
        const mgmt = new SdkManagementService({
          clientId: config.mgmtClientId,
          clientSecret: config.mgmtClientSecret,
          tsgId: config.mgmtTsgId,
          tokenEndpoint: config.mgmtTokenEndpoint,
        });

        const result = await revertTopic(mgmt, opts.profile, opts.name);

        if (opts.format === 'json') {
          console.log(JSON.stringify(result, null, 2));
        } else {
          console.log(`\n  Reverted: ${opts.name}`);
          console.log(`  Profile:  ${result.profileName}`);
          console.log(`  Deleted:  ${result.deleted.join(', ')}\n`);
        }
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd /Users/cdot/development/cdot65/prisma-airs-cli && pnpm test -- tests/unit/cli/topics-revert.spec.ts`
Expected: All 3 tests PASS

- [ ] **Step 5: Commit**

```bash
cd /Users/cdot/development/cdot65/prisma-airs-cli && git add src/cli/commands/topics-revert.ts tests/unit/cli/topics-revert.spec.ts && git commit -m "feat: add topics revert command to undo experiments"
```

---

### Task 8: Wire New Commands Into Runtime

**Files:**
- Modify: `src/cli/commands/runtime.ts`

- [ ] **Step 1: Replace old imports with new command imports**

In `src/cli/commands/runtime.ts`, replace lines 34-37:

```typescript
// Old:
import { registerGenerateCommand } from './generate.js';
import { registerListCommand } from './list.js';
import { registerReportCommand } from './report.js';
import { registerResumeCommand } from './resume.js';

// New:
import { registerApplyCommand } from './topics-apply.js';
import { registerCreateCommand } from './topics-create.js';
import { registerEvalCommand } from './topics-eval.js';
import { registerRevertCommand } from './topics-revert.js';
```

- [ ] **Step 2: Replace command registration at bottom of file**

Replace the existing `topics create` command block (lines 798-814) and the guardrail registration block (lines 856-860).

Remove the old inline `topics create` command (the one that takes `--config <path>`). Then replace lines 856-860:

```typescript
// Old:
  registerGenerateCommand(topics);
  registerResumeCommand(topics);
  registerReportCommand(topics);
  registerListCommand(topics, 'runs');

// New:
  registerCreateCommand(topics);
  registerApplyCommand(topics);
  registerEvalCommand(topics);
  registerRevertCommand(topics);
```

- [ ] **Step 3: Verify build compiles**

Run: `cd /Users/cdot/development/cdot65/prisma-airs-cli && pnpm tsc --noEmit`
Expected: No errors

- [ ] **Step 4: Commit**

```bash
cd /Users/cdot/development/cdot65/prisma-airs-cli && git add src/cli/commands/runtime.ts && git commit -m "feat: wire create/apply/eval/revert commands into runtime topics"
```

---

### Task 9: Remove Old Code

**Files:**
- Delete: `src/llm/`, `src/memory/`, `src/persistence/`, `src/core/loop.ts`
- Delete: `src/cli/commands/generate.ts`, `resume.ts`, `report.ts`, `list.ts`
- Delete: `src/cli/renderer/generate.ts`
- Delete: `tests/unit/llm/`, `tests/unit/memory/`, `tests/unit/persistence/`, `tests/unit/core/loop.spec.ts`, `tests/unit/report/`, `tests/integration/`
- Modify: `src/cli/renderer/index.ts`, `src/core/types.ts`, `src/index.ts`, `tests/helpers/mocks.ts`

- [ ] **Step 1: Delete old source directories and files**

```bash
cd /Users/cdot/development/cdot65/prisma-airs-cli && rm -rf src/llm src/memory src/persistence && rm src/core/loop.ts && rm src/cli/commands/generate.ts src/cli/commands/resume.ts src/cli/commands/report.ts src/cli/commands/list.ts && rm src/cli/renderer/generate.ts
```

- [ ] **Step 2: Delete corresponding tests**

```bash
cd /Users/cdot/development/cdot65/prisma-airs-cli && rm -rf tests/unit/llm tests/unit/memory tests/unit/persistence tests/unit/report tests/integration && rm tests/unit/core/loop.spec.ts
```

- [ ] **Step 3: Remove generate re-export from renderer index**

In `src/cli/renderer/index.ts`, remove `export * from './generate.js';`. Final file:

```typescript
export * from './audit.js';
export * from './common.js';
export * from './eval.js';
export * from './modelsecurity.js';
export * from './redteam.js';
export * from './runtime.js';
```

- [ ] **Step 4: Remove loop-specific types from `src/core/types.ts`**

Remove: `UserInput`, `AnalysisReport`, `IterationResult`, `RunState`, `LoopEvent` (entire union).

Keep: `CustomTopic`, `TestCase`, `CategoryBreakdown`, `TestResult`, `EfficacyMetrics`.

- [ ] **Step 5: Clean up library exports in `src/index.ts`**

Read the file and remove any re-exports of `llm`, `memory`, `persistence`, `loop`, or types that no longer exist.

- [ ] **Step 6: Clean up test mocks in `tests/helpers/mocks.ts`**

Remove: `mockAnalysis()`, `mockIterationResult()`, `mockRunState()`, `createMockPromptSetService()`.

Remove imports: `AnalysisReport`, `IterationResult`, `RunState` from the import block. Remove `PromptSetService` from the airs/types import.

Keep: `mockTopic()`, `mockTestCases()`, `createMockManagementService()`, `createMockScanService()`, `createMockAllowScanService()`, `mockMetrics()`, `mockTestResults()`.

- [ ] **Step 7: Verify build compiles**

Run: `cd /Users/cdot/development/cdot65/prisma-airs-cli && pnpm tsc --noEmit`
Expected: No errors

- [ ] **Step 8: Run all remaining tests**

Run: `cd /Users/cdot/development/cdot65/prisma-airs-cli && pnpm test`
Expected: All tests pass

- [ ] **Step 9: Commit**

```bash
cd /Users/cdot/development/cdot65/prisma-airs-cli && git add -A && git commit -m "refactor: remove LLM loop, memory, persistence — replaced by atomic commands"
```

---

### Task 10: Remove LangChain Dependencies and Clean Config

**Files:**
- Modify: `package.json`
- Modify: `src/config/schema.ts`
- Modify: `src/config/loader.ts`

- [ ] **Step 1: Remove LangChain packages**

```bash
cd /Users/cdot/development/cdot65/prisma-airs-cli && pnpm remove @langchain/anthropic @langchain/aws @langchain/core @langchain/google-genai @langchain/google-vertexai
```

- [ ] **Step 2: Remove LLM/memory config fields from schema**

Read `src/config/schema.ts`. Remove from the Zod schema: `llmProvider`, `llmModel`, `anthropicApiKey`, `googleApiKey`, `googleCloudProject`, `googleCloudLocation`, `awsRegion`, `awsAccessKeyId`, `awsSecretAccessKey`, `accumulateTests`, `maxAccumulatedTests`, `memoryEnabled`, `memoryDir`, `maxMemoryChars`, `dataDir`.

Keep: `airsApiKey`, `mgmtClientId`, `mgmtClientSecret`, `mgmtTsgId`, `mgmtEndpoint`, `mgmtTokenEndpoint`, `scanConcurrency`.

- [ ] **Step 3: Remove corresponding env mappings from loader**

In `src/config/loader.ts`, remove env mappings for deleted fields from `fromEnv()`. Remove `expandHome()` calls for `memoryDir` and `dataDir` in the return.

- [ ] **Step 4: Verify everything**

```bash
cd /Users/cdot/development/cdot65/prisma-airs-cli && pnpm install && pnpm tsc --noEmit && pnpm test && pnpm run lint
```
Expected: All pass

- [ ] **Step 5: Commit**

```bash
cd /Users/cdot/development/cdot65/prisma-airs-cli && git add package.json pnpm-lock.yaml src/config/schema.ts src/config/loader.ts && git commit -m "chore: remove LangChain deps and LLM/memory config fields"
```

---

### Task 11: Write program.md

**Files:**
- Create: `program.md` (in prisma-airs-cli repo root)

- [ ] **Step 1: Write agent instructions**

```markdown
# autoresearch: custom topic guardrail optimization

Autonomous loop to find the optimal custom topic guardrail configuration for Prisma AIRS.

## Setup

1. **Confirm inputs** with the user:
   - Security profile name (must already exist in AIRS)
   - Topic name and initial description
   - Initial example prompts (2-5 required)
   - Intent: `block` or `allow`
   - Path to prompt CSV file (columns: `prompt`, `expected`)
   - Whether a companion allow topic is needed (block-intent usually requires one)
2. **Verify AIRS credentials**: run `airs runtime topics list` — if it errors, credentials are missing.
3. **Initialize results.tsv** with header row:
   ```
   iteration	coverage	tpr	tnr	f1	status	description_summary
   ```

## Baseline

Create the topic and assign it to the profile:

```bash
# Create block topic
airs runtime topics create \
  --name "<topic-name>" \
  --description "<user-provided-description>" \
  --examples "<ex1>" "<ex2>" \
  --format json

# If block-intent, create allow companion
airs runtime topics create \
  --name "<companion-name>" \
  --description "<companion-description>" \
  --examples "<comp-ex1>" "<comp-ex2>" \
  --format json

# Apply both to profile
airs runtime topics apply --profile "<profile>" --name "<topic-name>" --intent block --format json
airs runtime topics apply --profile "<profile>" --name "<companion-name>" --intent allow --format json
```

Then evaluate:

```bash
airs runtime topics eval \
  --profile "<profile>" \
  --prompts <path-to-csv> \
  --topic "<topic-name>" \
  --format json
```

Record baseline metrics in `results.tsv`. This is iteration 0.

## Experiment Loop

LOOP FOREVER:

1. **Read the eval output** — focus on the `false_positives` and `false_negatives` arrays.
2. **Reason about misclassifications:**
   - FP = prompt was flagged but shouldn't have been. Description is too broad.
   - FN = prompt should have been flagged but wasn't. Description doesn't cover it or examples are insufficient.
3. **Craft an improved description and/or examples.** Rules:
   - AIRS uses **semantic similarity**, NOT logical constraints.
   - **Never** use exclusion language ("not X", "excluding Y", "no Z") — it increases FP by adding semantic overlap.
   - **Shorter descriptions outperform longer ones** (under 100 chars is a good target).
   - Make the positive definition more precise rather than adding exceptions.
   - 2-5 examples required, 250 bytes each, 1000 bytes combined (name + description + all examples).
   - Topic name stays fixed — only change description and examples.
4. **Update the topic definition:**
   ```bash
   airs runtime topics create \
     --name "<topic-name>" \
     --description "<new-description>" \
     --examples "<new-ex1>" "<new-ex2>" \
     --format json
   ```
   Note: `create` updates the existing topic when the name matches.
5. **Re-apply to profile** (picks up new revision):
   ```bash
   airs runtime topics apply --profile "<profile>" --name "<topic-name>" --intent block --format json
   ```
6. **Evaluate against the same static prompt set:**
   ```bash
   airs runtime topics eval \
     --profile "<profile>" \
     --prompts <path-to-csv> \
     --topic "<topic-name>" \
     --format json
   ```
7. **Decide:**
   - If **coverage improved** (higher than best so far): **keep**. Record in results.tsv.
   - If **coverage equal or worse**: **discard**. Re-apply the previous best definition (steps 4-5 with best-known description/examples), record as `discard`.
8. **Never stop.** Do not ask the user if you should continue. Run until manually interrupted.

## Logging

Append to `results.tsv` (tab-separated, untracked by git):

```
iteration	coverage	tpr	tnr	f1	status	description_summary
0	0.840	0.840	0.960	0.890	keep	baseline
1	0.880	0.880	0.960	0.910	keep	shortened description, added ammo example
2	0.860	0.860	0.940	0.890	discard	tried adding modifier example
```

## Platform Constraints

- Block-intent coverage ceiling: typically 40-50% due to vocabulary overlap.
- Allow-intent ceiling: typically 40-70%.
- High-sensitivity domains (explosives, weapons) may hit AIRS built-in safety that overrides custom definitions.
- If coverage plateaus for 5+ iterations, try a fundamentally different description angle rather than incremental tweaks.
- The companion allow topic may also need refinement — if FP is high, the allow companion's description may be too narrow.

## When the User Returns

Report:
1. Best coverage achieved and the iteration that produced it.
2. The best-performing topic definition (description + examples).
3. Total iterations attempted, keeps vs discards.
```

- [ ] **Step 2: Commit**

```bash
cd /Users/cdot/development/cdot65/prisma-airs-cli && git add program.md && git commit -m "feat: add program.md agent instructions for autoresearch loop"
```

---

### Task 12: Update CLAUDE.md

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Update project summary**

Replace LLM-loop description with new atomic-command architecture. Key changes:
- Summary: remove "LLM-driven guardrail generation with iterative refinement" and "cross-run memory" — replace with "atomic topic commands (create, apply, eval, revert) for agent-driven optimization"
- Directory structure: remove `src/llm/`, `src/memory/`, `src/persistence/`, `src/core/loop.ts`, old commands. Add `src/core/prompt-loader.ts`, new command files, `src/cli/renderer/eval.ts`.
- Architecture section: replace Core Loop section with new commands section documenting `create`, `apply`, `eval`, `revert`
- Remove LLM Service, Memory System, Persistence sections
- Remove LLM-related env vars (ANTHROPIC_API_KEY, GOOGLE_*, AWS_*, LLM_*, MEMORY_*, DATA_DIR, ACCUMULATE_*)
- Tech stack: remove "LangChain.js w/ structured output"
- Add reference to `program.md` for agent-driven optimization workflow

- [ ] **Step 2: Verify accuracy against file tree**

Run: `cd /Users/cdot/development/cdot65/prisma-airs-cli && ls src/`
Confirm no stale references in CLAUDE.md.

- [ ] **Step 3: Commit**

```bash
cd /Users/cdot/development/cdot65/prisma-airs-cli && git add CLAUDE.md && git commit -m "docs: update CLAUDE.md for autoresearch refactor"
```
