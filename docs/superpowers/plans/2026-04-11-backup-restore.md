# Backup & Restore Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `airs backup targets` and `airs restore targets` CLI commands for exporting/importing redteam target configurations to/from local JSON/YAML files.

**Architecture:** Shared `src/backup/` module with I/O helpers (write/read backup files, sanitize filenames). Thin CLI command handlers in `src/cli/commands/backup.ts` and `src/cli/commands/restore.ts` orchestrate the flow. Renderer in `src/cli/renderer/backup.ts` for terminal output.

**Tech Stack:** TypeScript, Commander.js, chalk, js-yaml (new dep), vitest

---

### Task 1: Add js-yaml dependency

**Files:**
- Modify: `package.json:41-56` (dependencies)

- [ ] **Step 1: Install js-yaml and its types**

```bash
pnpm add js-yaml && pnpm add -D @types/js-yaml
```

- [ ] **Step 2: Verify installation**

```bash
pnpm ls js-yaml @types/js-yaml
```

Expected: both packages listed

- [ ] **Step 3: Commit**

```bash
git add package.json pnpm-lock.yaml
git commit -m "chore: add js-yaml dependency for backup/restore YAML support"
```

---

### Task 2: Create backup types

**Files:**
- Create: `src/backup/types.ts`
- Test: `tests/unit/backup/io.spec.ts` (type usage validated by compile)

- [ ] **Step 1: Create `src/backup/types.ts`**

```typescript
/** Supported backup serialization formats. */
export type BackupFormat = 'json' | 'yaml';

/** Discriminator for multi-resource backup directories. */
export type ResourceType = 'redteam-target';

/** Envelope wrapping any backed-up resource. */
export interface BackupEnvelope<T> {
  version: string;
  resourceType: ResourceType;
  exportedAt: string;
  data: T;
}

/** Per-target result reported after a backup run. */
export interface BackupResult {
  name: string;
  filename: string;
  status: 'ok' | 'failed';
  error?: string;
}

/** Per-target result reported after a restore run. */
export interface RestoreResult {
  name: string;
  action: 'created' | 'updated' | 'skipped' | 'failed';
  error?: string;
}
```

- [ ] **Step 2: Type-check**

Run: `pnpm tsc --noEmit`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src/backup/types.ts
git commit -m "feat(backup): add backup envelope and result types"
```

---

### Task 3: Create backup I/O utilities — write tests

**Files:**
- Create: `tests/unit/backup/io.spec.ts`

- [ ] **Step 1: Write failing tests for `sanitizeFilename`**

```typescript
import { describe, expect, it } from 'vitest';
import { sanitizeFilename } from '../../../src/backup/io.js';

describe('sanitizeFilename', () => {
  it('lowercases and replaces spaces with hyphens', () => {
    expect(sanitizeFilename('My Target Name')).toBe('my-target-name');
  });

  it('replaces non-alphanumeric chars with hyphens', () => {
    expect(sanitizeFilename('test/target@v2!')).toBe('test-target-v2-');
  });

  it('collapses consecutive hyphens', () => {
    expect(sanitizeFilename('a---b___c')).toBe('a-b-c');
  });

  it('trims leading/trailing hyphens', () => {
    expect(sanitizeFilename('--hello--')).toBe('hello');
  });

  it('handles empty string', () => {
    expect(sanitizeFilename('')).toBe('unnamed');
  });
});
```

- [ ] **Step 2: Write failing tests for `resolveOutputDir`**

Append to the same file:

```typescript
import { resolveOutputDir } from '../../../src/backup/io.js';
import path from 'node:path';

describe('resolveOutputDir', () => {
  it('returns user-specified dir as absolute path', () => {
    const result = resolveOutputDir('/tmp/my-backups', 'targets');
    expect(result).toBe('/tmp/my-backups');
  });

  it('resolves relative user dir against cwd', () => {
    const result = resolveOutputDir('./backups', 'targets');
    expect(result).toBe(path.resolve('./backups'));
  });

  it('uses default subdir when no user dir given', () => {
    const result = resolveOutputDir(undefined, 'targets');
    expect(result).toBe(path.resolve('./airs-backup/targets'));
  });
});
```

- [ ] **Step 3: Write failing tests for `writeBackupFile` and `readBackupFile`**

Append to the same file:

```typescript
import { writeBackupFile, readBackupFile } from '../../../src/backup/io.js';
import type { BackupEnvelope } from '../../../src/backup/types.js';
import fs from 'node:fs';
import os from 'node:os';

describe('writeBackupFile + readBackupFile', () => {
  const envelope: BackupEnvelope<{ name: string }> = {
    version: '1',
    resourceType: 'redteam-target',
    exportedAt: '2026-04-11T00:00:00.000Z',
    data: { name: 'test-target' },
  };

  it('round-trips JSON', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'backup-test-'));
    writeBackupFile(dir, 'test-target', envelope, 'json');
    const filePath = path.join(dir, 'test-target.json');
    expect(fs.existsSync(filePath)).toBe(true);
    const loaded = readBackupFile<{ name: string }>(filePath);
    expect(loaded).toEqual(envelope);
    fs.rmSync(dir, { recursive: true });
  });

  it('round-trips YAML', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'backup-test-'));
    writeBackupFile(dir, 'test-target', envelope, 'yaml');
    const filePath = path.join(dir, 'test-target.yaml');
    expect(fs.existsSync(filePath)).toBe(true);
    const loaded = readBackupFile<{ name: string }>(filePath);
    expect(loaded).toEqual(envelope);
    fs.rmSync(dir, { recursive: true });
  });

  it('creates directories recursively', () => {
    const dir = path.join(fs.mkdtempSync(path.join(os.tmpdir(), 'backup-test-')), 'a', 'b');
    writeBackupFile(dir, 'test', envelope, 'json');
    expect(fs.existsSync(path.join(dir, 'test.json'))).toBe(true);
    fs.rmSync(path.resolve(dir, '../..'), { recursive: true });
  });

  it('throws on invalid envelope (missing version)', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'backup-test-'));
    const filePath = path.join(dir, 'bad.json');
    fs.writeFileSync(filePath, JSON.stringify({ data: {} }));
    expect(() => readBackupFile(filePath)).toThrow('Invalid backup file');
    fs.rmSync(dir, { recursive: true });
  });

  it('throws on unsupported file extension', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'backup-test-'));
    const filePath = path.join(dir, 'bad.txt');
    fs.writeFileSync(filePath, 'hello');
    expect(() => readBackupFile(filePath)).toThrow('Unsupported file format');
    fs.rmSync(dir, { recursive: true });
  });
});
```

- [ ] **Step 4: Write failing tests for `readBackupDir`**

Append to the same file:

```typescript
import { readBackupDir } from '../../../src/backup/io.js';

describe('readBackupDir', () => {
  it('reads all matching files from directory', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'backup-test-'));
    const env1: BackupEnvelope<{ name: string }> = {
      version: '1',
      resourceType: 'redteam-target',
      exportedAt: '2026-04-11T00:00:00.000Z',
      data: { name: 'target-a' },
    };
    const env2: BackupEnvelope<{ name: string }> = {
      version: '1',
      resourceType: 'redteam-target',
      exportedAt: '2026-04-11T00:00:00.000Z',
      data: { name: 'target-b' },
    };
    writeBackupFile(dir, 'target-a', env1, 'json');
    writeBackupFile(dir, 'target-b', env2, 'yaml');
    const results = readBackupDir<{ name: string }>(dir, 'redteam-target');
    expect(results).toHaveLength(2);
    expect(results.map((r) => r.data.name).sort()).toEqual(['target-a', 'target-b']);
    fs.rmSync(dir, { recursive: true });
  });

  it('filters by resourceType', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'backup-test-'));
    const env: BackupEnvelope<{ name: string }> = {
      version: '1',
      resourceType: 'redteam-target',
      exportedAt: '2026-04-11T00:00:00.000Z',
      data: { name: 'target-a' },
    };
    writeBackupFile(dir, 'target-a', env, 'json');
    // Write a file with a different resourceType manually
    fs.writeFileSync(
      path.join(dir, 'other.json'),
      JSON.stringify({ version: '1', resourceType: 'other-thing', exportedAt: '', data: {} }),
    );
    const results = readBackupDir<{ name: string }>(dir, 'redteam-target');
    expect(results).toHaveLength(1);
    expect(results[0].data.name).toBe('target-a');
    fs.rmSync(dir, { recursive: true });
  });
});
```

- [ ] **Step 5: Run tests to verify they fail**

Run: `pnpm test -- tests/unit/backup/io.spec.ts`
Expected: FAIL — `Cannot find module '../../../src/backup/io.js'`

- [ ] **Step 6: Commit**

```bash
git add tests/unit/backup/io.spec.ts
git commit -m "test(backup): add failing tests for backup I/O utilities"
```

---

### Task 4: Implement backup I/O utilities

**Files:**
- Create: `src/backup/io.ts`
- Create: `src/backup/index.ts`

- [ ] **Step 1: Create `src/backup/io.ts`**

```typescript
import fs from 'node:fs';
import path from 'node:path';
import yaml from 'js-yaml';
import type { BackupEnvelope, BackupFormat, ResourceType } from './types.js';

/**
 * Sanitize a resource name into a filesystem-safe filename (no extension).
 * Lowercases, replaces non-alphanumeric with hyphens, collapses runs, trims.
 */
export function sanitizeFilename(name: string): string {
  if (!name) return 'unnamed';
  const sanitized = name
    .toLowerCase()
    .replace(/[^a-z0-9-]/g, '-')
    .replace(/-{2,}/g, '-')
    .replace(/^-+|-+$/g, '');
  return sanitized || 'unnamed';
}

/** Resolve output directory — user-specified or default `./airs-backup/<subdir>/`. */
export function resolveOutputDir(userDir: string | undefined, defaultSubdir: string): string {
  if (userDir) return path.resolve(userDir);
  return path.resolve('./airs-backup', defaultSubdir);
}

/** Serialize envelope and write to `dir/filename.{json|yaml}`. Creates dir if needed. */
export function writeBackupFile<T>(
  dir: string,
  filename: string,
  envelope: BackupEnvelope<T>,
  format: BackupFormat,
): void {
  fs.mkdirSync(dir, { recursive: true });
  const ext = format === 'yaml' ? 'yaml' : 'json';
  const filePath = path.join(dir, `${filename}.${ext}`);
  const content =
    format === 'yaml'
      ? yaml.dump(envelope, { lineWidth: -1, noRefs: true })
      : JSON.stringify(envelope, null, 2) + '\n';
  fs.writeFileSync(filePath, content, 'utf-8');
}

/** Read and parse a single backup file. Detects format from extension. */
export function readBackupFile<T>(filePath: string): BackupEnvelope<T> {
  const ext = path.extname(filePath).toLowerCase();
  if (ext !== '.json' && ext !== '.yaml' && ext !== '.yml') {
    throw new Error(`Unsupported file format: ${ext} (expected .json, .yaml, or .yml)`);
  }
  const raw = fs.readFileSync(filePath, 'utf-8');
  const parsed = ext === '.json' ? JSON.parse(raw) : yaml.load(raw);
  if (!parsed || typeof parsed !== 'object' || !('version' in parsed) || !('data' in parsed)) {
    throw new Error(`Invalid backup file: ${filePath} (missing version or data)`);
  }
  return parsed as BackupEnvelope<T>;
}

/** Read all backup files from a directory, filtering by resourceType. */
export function readBackupDir<T>(dirPath: string, resourceType: ResourceType): BackupEnvelope<T>[] {
  const files = fs
    .readdirSync(dirPath)
    .filter((f) => /\.(json|ya?ml)$/i.test(f))
    .map((f) => path.join(dirPath, f));
  const envelopes: BackupEnvelope<T>[] = [];
  for (const filePath of files) {
    const envelope = readBackupFile<T>(filePath);
    if (envelope.resourceType === resourceType) {
      envelopes.push(envelope);
    }
  }
  return envelopes;
}
```

- [ ] **Step 2: Create `src/backup/index.ts`**

```typescript
export * from './io.js';
export * from './types.js';
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `pnpm test -- tests/unit/backup/io.spec.ts`
Expected: ALL PASS

- [ ] **Step 4: Lint**

Run: `pnpm run lint:fix`

- [ ] **Step 5: Commit**

```bash
git add src/backup/
git commit -m "feat(backup): implement backup I/O utilities"
```

---

### Task 5: Create backup renderer — write tests

**Files:**
- Create: `tests/unit/cli/backup-renderer.spec.ts`

- [ ] **Step 1: Write failing tests for `renderBackupSummary`**

```typescript
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { BackupResult, RestoreResult } from '../../../src/backup/types.js';

// Capture console.log output
let output: string[] = [];
const originalLog = console.log;

describe('renderBackupSummary', () => {
  afterEach(() => {
    output = [];
    console.log = originalLog;
  });

  it('renders backup results with count and directory', async () => {
    console.log = (...args: unknown[]) => output.push(args.join(' '));
    const { renderBackupSummary } = await import('../../../src/cli/renderer/backup.js');
    const results: BackupResult[] = [
      { name: 'target-a', filename: 'target-a.json', status: 'ok' },
      { name: 'target-b', filename: 'target-b.json', status: 'ok' },
    ];
    renderBackupSummary(results, '/tmp/backups');
    const text = output.join('\n');
    expect(text).toContain('target-a');
    expect(text).toContain('target-b');
    expect(text).toContain('2');
    expect(text).toContain('/tmp/backups');
  });

  it('renders failed results', async () => {
    console.log = (...args: unknown[]) => output.push(args.join(' '));
    const { renderBackupSummary } = await import('../../../src/cli/renderer/backup.js');
    const results: BackupResult[] = [
      { name: 'target-a', filename: 'target-a.json', status: 'failed', error: 'boom' },
    ];
    renderBackupSummary(results, '/tmp/backups');
    const text = output.join('\n');
    expect(text).toContain('target-a');
    expect(text).toContain('boom');
  });
});
```

- [ ] **Step 2: Write failing tests for `renderRestoreSummary`**

Append to the same file:

```typescript
describe('renderRestoreSummary', () => {
  afterEach(() => {
    output = [];
    console.log = originalLog;
  });

  it('renders restore results with action totals', async () => {
    console.log = (...args: unknown[]) => output.push(args.join(' '));
    const { renderRestoreSummary } = await import('../../../src/cli/renderer/backup.js');
    const results: RestoreResult[] = [
      { name: 'target-a', action: 'created' },
      { name: 'target-b', action: 'updated' },
      { name: 'target-c', action: 'skipped' },
    ];
    renderRestoreSummary(results);
    const text = output.join('\n');
    expect(text).toContain('target-a');
    expect(text).toContain('created');
    expect(text).toContain('target-b');
    expect(text).toContain('updated');
    expect(text).toContain('target-c');
    expect(text).toContain('skipped');
  });

  it('renders failed results with error', async () => {
    console.log = (...args: unknown[]) => output.push(args.join(' '));
    const { renderRestoreSummary } = await import('../../../src/cli/renderer/backup.js');
    const results: RestoreResult[] = [
      { name: 'target-a', action: 'failed', error: 'API error' },
    ];
    renderRestoreSummary(results);
    const text = output.join('\n');
    expect(text).toContain('target-a');
    expect(text).toContain('failed');
    expect(text).toContain('API error');
  });
});
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `pnpm test -- tests/unit/cli/backup-renderer.spec.ts`
Expected: FAIL — cannot find module `../../../src/cli/renderer/backup.js`

- [ ] **Step 4: Commit**

```bash
git add tests/unit/cli/backup-renderer.spec.ts
git commit -m "test(backup): add failing tests for backup/restore renderers"
```

---

### Task 6: Implement backup renderer

**Files:**
- Create: `src/cli/renderer/backup.ts`
- Modify: `src/cli/renderer/index.ts:1-6`

- [ ] **Step 1: Create `src/cli/renderer/backup.ts`**

```typescript
import chalk from 'chalk';
import type { BackupResult, RestoreResult } from '../../backup/types.js';

/** Render header for backup/restore commands. */
export function renderBackupHeader(): void {
  console.log(chalk.bold('\n  Prisma AIRS — Backup & Restore\n'));
}

/** Render backup run summary. */
export function renderBackupSummary(results: BackupResult[], outputDir: string): void {
  const ok = results.filter((r) => r.status === 'ok');
  const failed = results.filter((r) => r.status === 'failed');

  if (ok.length > 0) {
    console.log(chalk.bold(`\n  Backed up ${ok.length} target(s) to ${outputDir}:\n`));
    for (const r of ok) {
      console.log(`    ${chalk.green('✓')} ${r.name} → ${chalk.dim(r.filename)}`);
    }
  }

  if (failed.length > 0) {
    console.log(chalk.bold(`\n  Failed (${failed.length}):\n`));
    for (const r of failed) {
      console.log(`    ${chalk.red('✗')} ${r.name}: ${r.error}`);
    }
  }

  console.log();
}

/** Render restore run summary. */
export function renderRestoreSummary(results: RestoreResult[]): void {
  const groups = { created: 0, updated: 0, skipped: 0, failed: 0 };
  for (const r of results) groups[r.action]++;

  console.log(chalk.bold('\n  Restore results:\n'));
  for (const r of results) {
    const icon =
      r.action === 'failed'
        ? chalk.red('✗')
        : r.action === 'skipped'
          ? chalk.yellow('○')
          : chalk.green('✓');
    const suffix = r.error ? `: ${r.error}` : '';
    console.log(`    ${icon} ${r.name} — ${r.action}${suffix}`);
  }

  const parts: string[] = [];
  if (groups.created) parts.push(`${groups.created} created`);
  if (groups.updated) parts.push(`${groups.updated} updated`);
  if (groups.skipped) parts.push(`${groups.skipped} skipped`);
  if (groups.failed) parts.push(chalk.red(`${groups.failed} failed`));
  console.log(`\n  Total: ${parts.join(', ')}\n`);
}
```

- [ ] **Step 2: Add to barrel exports in `src/cli/renderer/index.ts`**

Add this line after the existing exports:

```typescript
export * from './backup.js';
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `pnpm test -- tests/unit/cli/backup-renderer.spec.ts`
Expected: ALL PASS

- [ ] **Step 4: Lint**

Run: `pnpm run lint:fix`

- [ ] **Step 5: Commit**

```bash
git add src/cli/renderer/backup.ts src/cli/renderer/index.ts
git commit -m "feat(backup): implement backup/restore renderers"
```

---

### Task 7: Implement backup command — write tests

**Files:**
- Create: `tests/unit/cli/backup.spec.ts`

- [ ] **Step 1: Write failing tests for backup targets command logic**

```typescript
import { afterEach, describe, expect, it, vi } from 'vitest';

// Mock the airs service
const mockListTargets = vi.fn();
const mockGetTarget = vi.fn();
vi.mock('../../../src/airs/redteam.js', () => ({
  SdkRedTeamService: vi.fn().mockImplementation(() => ({
    listTargets: mockListTargets,
    getTarget: mockGetTarget,
  })),
}));

// Mock config loader
vi.mock('../../../src/config/loader.js', () => ({
  loadConfig: vi.fn().mockResolvedValue({
    mgmtClientId: 'id',
    mgmtClientSecret: 'secret',
    mgmtTsgId: 'tsg',
    mgmtTokenEndpoint: 'https://token',
  }),
}));

import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { backupTargets } from '../../../src/cli/commands/backup.js';

describe('backupTargets', () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it('backs up all targets as JSON files', async () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'backup-cmd-'));
    mockListTargets.mockResolvedValue([
      { uuid: 'u1', name: 'Target A', status: 'active', targetType: 'OPENAI', active: true },
    ]);
    mockGetTarget.mockResolvedValue({
      uuid: 'u1',
      name: 'Target A',
      status: 'active',
      targetType: 'OPENAI',
      active: true,
      connectionParams: { api_key: 'sk-123' },
      background: { industry: 'tech' },
    });

    const results = await backupTargets({ outputDir: dir, format: 'json' });

    expect(results).toHaveLength(1);
    expect(results[0].status).toBe('ok');
    const written = JSON.parse(fs.readFileSync(path.join(dir, 'target-a.json'), 'utf-8'));
    expect(written.version).toBe('1');
    expect(written.resourceType).toBe('redteam-target');
    expect(written.data.name).toBe('Target A');
    expect(written.data.target_type).toBe('OPENAI');
    expect(written.data.connection_params.api_key).toBe('sk-123');
    // Server-only fields stripped
    expect(written.data.uuid).toBeUndefined();
    expect(written.data.status).toBeUndefined();
    expect(written.data.active).toBeUndefined();
    fs.rmSync(dir, { recursive: true });
  });

  it('backs up single target by name', async () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'backup-cmd-'));
    mockListTargets.mockResolvedValue([
      { uuid: 'u1', name: 'Target A', status: 'active', targetType: 'OPENAI', active: true },
      { uuid: 'u2', name: 'Target B', status: 'active', targetType: 'REST', active: true },
    ]);
    mockGetTarget.mockResolvedValue({
      uuid: 'u1',
      name: 'Target A',
      status: 'active',
      targetType: 'OPENAI',
      active: true,
      connectionParams: { api_key: 'sk-123' },
    });

    const results = await backupTargets({ outputDir: dir, format: 'json', name: 'Target A' });

    expect(results).toHaveLength(1);
    expect(mockGetTarget).toHaveBeenCalledWith('u1');
    expect(mockGetTarget).toHaveBeenCalledTimes(1);
    fs.rmSync(dir, { recursive: true });
  });

  it('errors when named target not found', async () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'backup-cmd-'));
    mockListTargets.mockResolvedValue([
      { uuid: 'u1', name: 'Target A', status: 'active', targetType: 'OPENAI', active: true },
    ]);

    await expect(
      backupTargets({ outputDir: dir, format: 'json', name: 'Nonexistent' }),
    ).rejects.toThrow('Target not found: Nonexistent');
    fs.rmSync(dir, { recursive: true });
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `pnpm test -- tests/unit/cli/backup.spec.ts`
Expected: FAIL — cannot find module `../../../src/cli/commands/backup.js`

- [ ] **Step 3: Commit**

```bash
git add tests/unit/cli/backup.spec.ts
git commit -m "test(backup): add failing tests for backup targets command"
```

---

### Task 8: Implement backup command

**Files:**
- Create: `src/cli/commands/backup.ts`
- Modify: `src/cli/index.ts:9-11` (imports), `src/cli/index.ts:38-40` (registration)

- [ ] **Step 1: Create `src/cli/commands/backup.ts`**

```typescript
import { Command } from 'commander';
import { SdkRedTeamService } from '../../airs/redteam.js';
import type { RedTeamTargetDetail } from '../../airs/types.js';
import { resolveOutputDir, sanitizeFilename, writeBackupFile } from '../../backup/io.js';
import type { BackupEnvelope, BackupFormat, BackupResult } from '../../backup/types.js';
import { loadConfig } from '../../config/loader.js';
import { renderBackupHeader, renderBackupSummary, renderError } from '../renderer/index.js';

async function createRedTeamService(): Promise<SdkRedTeamService> {
  const config = await loadConfig();
  return new SdkRedTeamService({
    clientId: config.mgmtClientId,
    clientSecret: config.mgmtClientSecret,
    tsgId: config.mgmtTsgId,
    tokenEndpoint: config.mgmtTokenEndpoint,
  });
}

/** Convert a RedTeamTargetDetail to the create-request shape (strip server fields, snake_case). */
function toBackupData(
  target: RedTeamTargetDetail,
): Record<string, unknown> {
  const data: Record<string, unknown> = {
    name: target.name,
    target_type: target.targetType,
  };
  if (target.connectionParams) data.connection_params = target.connectionParams;
  if (target.background) data.background = target.background;
  if (target.additionalContext) data.additional_context = target.additionalContext;
  if (target.metadata) data.metadata = target.metadata;
  return data;
}

/** Core backup logic — exported for testability. */
export async function backupTargets(opts: {
  outputDir: string;
  format: BackupFormat;
  name?: string;
}): Promise<BackupResult[]> {
  const service = await createRedTeamService();
  const allTargets = await service.listTargets();

  let uuids: Array<{ uuid: string; name: string }>;
  if (opts.name) {
    const match = allTargets.find((t) => t.name === opts.name);
    if (!match) throw new Error(`Target not found: ${opts.name}`);
    uuids = [{ uuid: match.uuid, name: match.name }];
  } else {
    uuids = allTargets.map((t) => ({ uuid: t.uuid, name: t.name }));
  }

  const dir = resolveOutputDir(opts.outputDir, 'targets');
  const results: BackupResult[] = [];

  for (const entry of uuids) {
    try {
      const detail = await service.getTarget(entry.uuid);
      const envelope: BackupEnvelope<Record<string, unknown>> = {
        version: '1',
        resourceType: 'redteam-target',
        exportedAt: new Date().toISOString(),
        data: toBackupData(detail),
      };
      const filename = sanitizeFilename(entry.name);
      writeBackupFile(dir, filename, envelope, opts.format);
      results.push({ name: entry.name, filename: `${filename}.${opts.format === 'yaml' ? 'yaml' : 'json'}`, status: 'ok' });
    } catch (err) {
      results.push({
        name: entry.name,
        filename: '',
        status: 'failed',
        error: err instanceof Error ? err.message : String(err),
      });
    }
  }

  return results;
}

export function registerBackupCommand(program: Command): void {
  const backup = program.command('backup').description('Backup AIRS configuration to local files');

  backup
    .command('targets')
    .description('Backup red team targets to local JSON/YAML files')
    .option('--output-dir <path>', 'Output directory', undefined)
    .option('--format <format>', 'Output format: json or yaml', 'json')
    .option('--name <targetName>', 'Backup a single target by name')
    .action(async (opts) => {
      try {
        renderBackupHeader();
        const format = opts.format as BackupFormat;
        if (format !== 'json' && format !== 'yaml') {
          throw new Error(`Invalid format: ${format} (expected json or yaml)`);
        }
        const dir = resolveOutputDir(opts.outputDir, 'targets');
        const results = await backupTargets({
          outputDir: opts.outputDir,
          format,
          name: opts.name,
        });
        renderBackupSummary(results, dir);
        const failed = results.filter((r) => r.status === 'failed').length;
        if (failed > 0) process.exit(1);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });
}
```

- [ ] **Step 2: Register in `src/cli/index.ts`**

Add import alongside existing command imports:

```typescript
import { registerBackupCommand } from './commands/backup.js';
```

Add registration after existing command registrations:

```typescript
registerBackupCommand(program);
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `pnpm test -- tests/unit/cli/backup.spec.ts`
Expected: ALL PASS

- [ ] **Step 4: Type-check**

Run: `pnpm tsc --noEmit`
Expected: PASS

- [ ] **Step 5: Lint**

Run: `pnpm run lint:fix`

- [ ] **Step 6: Commit**

```bash
git add src/cli/commands/backup.ts src/cli/index.ts
git commit -m "feat(backup): implement backup targets command"
```

---

### Task 9: Implement restore command — write tests

**Files:**
- Create: `tests/unit/cli/restore.spec.ts`

- [ ] **Step 1: Write failing tests for restore targets command logic**

```typescript
import { afterEach, describe, expect, it, vi } from 'vitest';

const mockListTargets = vi.fn();
const mockCreateTarget = vi.fn();
const mockUpdateTarget = vi.fn();
vi.mock('../../../src/airs/redteam.js', () => ({
  SdkRedTeamService: vi.fn().mockImplementation(() => ({
    listTargets: mockListTargets,
    createTarget: mockCreateTarget,
    updateTarget: mockUpdateTarget,
  })),
}));

vi.mock('../../../src/config/loader.js', () => ({
  loadConfig: vi.fn().mockResolvedValue({
    mgmtClientId: 'id',
    mgmtClientSecret: 'secret',
    mgmtTsgId: 'tsg',
    mgmtTokenEndpoint: 'https://token',
  }),
}));

import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { writeBackupFile } from '../../../src/backup/io.js';
import type { BackupEnvelope } from '../../../src/backup/types.js';
import { restoreTargets } from '../../../src/cli/commands/restore.js';

const sampleEnvelope: BackupEnvelope<Record<string, unknown>> = {
  version: '1',
  resourceType: 'redteam-target',
  exportedAt: '2026-04-11T00:00:00.000Z',
  data: {
    name: 'Target A',
    target_type: 'OPENAI',
    connection_params: { api_key: 'sk-123' },
  },
};

describe('restoreTargets', () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it('creates new target from single file', async () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'restore-'));
    writeBackupFile(dir, 'target-a', sampleEnvelope, 'json');
    mockListTargets.mockResolvedValue([]);
    mockCreateTarget.mockResolvedValue({ uuid: 'new-uuid', name: 'Target A' });

    const results = await restoreTargets({
      file: path.join(dir, 'target-a.json'),
    });

    expect(results).toHaveLength(1);
    expect(results[0].action).toBe('created');
    expect(mockCreateTarget).toHaveBeenCalledWith(sampleEnvelope.data, undefined);
    fs.rmSync(dir, { recursive: true });
  });

  it('skips existing target without --overwrite', async () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'restore-'));
    writeBackupFile(dir, 'target-a', sampleEnvelope, 'json');
    mockListTargets.mockResolvedValue([
      { uuid: 'u1', name: 'Target A', status: 'active', targetType: 'OPENAI', active: true },
    ]);

    const results = await restoreTargets({
      file: path.join(dir, 'target-a.json'),
    });

    expect(results).toHaveLength(1);
    expect(results[0].action).toBe('skipped');
    expect(mockCreateTarget).not.toHaveBeenCalled();
    fs.rmSync(dir, { recursive: true });
  });

  it('updates existing target with --overwrite', async () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'restore-'));
    writeBackupFile(dir, 'target-a', sampleEnvelope, 'json');
    mockListTargets.mockResolvedValue([
      { uuid: 'u1', name: 'Target A', status: 'active', targetType: 'OPENAI', active: true },
    ]);
    mockUpdateTarget.mockResolvedValue({ uuid: 'u1', name: 'Target A' });

    const results = await restoreTargets({
      file: path.join(dir, 'target-a.json'),
      overwrite: true,
    });

    expect(results).toHaveLength(1);
    expect(results[0].action).toBe('updated');
    expect(mockUpdateTarget).toHaveBeenCalledWith('u1', sampleEnvelope.data, undefined);
    fs.rmSync(dir, { recursive: true });
  });

  it('restores all files from directory', async () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'restore-'));
    const env2 = { ...sampleEnvelope, data: { ...sampleEnvelope.data, name: 'Target B' } };
    writeBackupFile(dir, 'target-a', sampleEnvelope, 'json');
    writeBackupFile(dir, 'target-b', env2, 'yaml');
    mockListTargets.mockResolvedValue([]);
    mockCreateTarget
      .mockResolvedValueOnce({ uuid: 'new-1', name: 'Target A' })
      .mockResolvedValueOnce({ uuid: 'new-2', name: 'Target B' });

    const results = await restoreTargets({ inputDir: dir });

    expect(results).toHaveLength(2);
    expect(results.every((r) => r.action === 'created')).toBe(true);
    fs.rmSync(dir, { recursive: true });
  });

  it('collects errors without aborting batch', async () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'restore-'));
    const env2 = { ...sampleEnvelope, data: { ...sampleEnvelope.data, name: 'Target B' } };
    writeBackupFile(dir, 'target-a', sampleEnvelope, 'json');
    writeBackupFile(dir, 'target-b', env2, 'json');
    mockListTargets.mockResolvedValue([]);
    mockCreateTarget
      .mockRejectedValueOnce(new Error('API error'))
      .mockResolvedValueOnce({ uuid: 'new-2', name: 'Target B' });

    const results = await restoreTargets({ inputDir: dir });

    expect(results).toHaveLength(2);
    const failed = results.find((r) => r.action === 'failed');
    const created = results.find((r) => r.action === 'created');
    expect(failed).toBeDefined();
    expect(failed!.error).toBe('API error');
    expect(created).toBeDefined();
    fs.rmSync(dir, { recursive: true });
  });

  it('passes --validate to service', async () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'restore-'));
    writeBackupFile(dir, 'target-a', sampleEnvelope, 'json');
    mockListTargets.mockResolvedValue([]);
    mockCreateTarget.mockResolvedValue({ uuid: 'new-uuid', name: 'Target A' });

    await restoreTargets({
      file: path.join(dir, 'target-a.json'),
      validate: true,
    });

    expect(mockCreateTarget).toHaveBeenCalledWith(sampleEnvelope.data, { validate: true });
    fs.rmSync(dir, { recursive: true });
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `pnpm test -- tests/unit/cli/restore.spec.ts`
Expected: FAIL — cannot find module `../../../src/cli/commands/restore.js`

- [ ] **Step 3: Commit**

```bash
git add tests/unit/cli/restore.spec.ts
git commit -m "test(backup): add failing tests for restore targets command"
```

---

### Task 10: Implement restore command

**Files:**
- Create: `src/cli/commands/restore.ts`
- Modify: `src/cli/index.ts` (import + registration)

- [ ] **Step 1: Create `src/cli/commands/restore.ts`**

```typescript
import { Command } from 'commander';
import { SdkRedTeamService } from '../../airs/redteam.js';
import type {
  RedTeamTargetCreateRequest,
  RedTeamTargetUpdateRequest,
} from '../../airs/types.js';
import { readBackupDir, readBackupFile } from '../../backup/io.js';
import type { BackupEnvelope, RestoreResult } from '../../backup/types.js';
import { loadConfig } from '../../config/loader.js';
import { renderBackupHeader, renderError, renderRestoreSummary } from '../renderer/index.js';

async function createRedTeamService(): Promise<SdkRedTeamService> {
  const config = await loadConfig();
  return new SdkRedTeamService({
    clientId: config.mgmtClientId,
    clientSecret: config.mgmtClientSecret,
    tsgId: config.mgmtTsgId,
    tokenEndpoint: config.mgmtTokenEndpoint,
  });
}

/** Core restore logic — exported for testability. */
export async function restoreTargets(opts: {
  file?: string;
  inputDir?: string;
  overwrite?: boolean;
  validate?: boolean;
}): Promise<RestoreResult[]> {
  const service = await createRedTeamService();

  let envelopes: BackupEnvelope<Record<string, unknown>>[];
  if (opts.file) {
    const env = readBackupFile<Record<string, unknown>>(opts.file);
    if (env.version !== '1' || env.resourceType !== 'redteam-target') {
      throw new Error(
        `Invalid backup: version=${env.version}, resourceType=${env.resourceType}`,
      );
    }
    envelopes = [env];
  } else if (opts.inputDir) {
    envelopes = readBackupDir<Record<string, unknown>>(opts.inputDir, 'redteam-target');
  } else {
    throw new Error('Specify --file or --input-dir');
  }

  if (envelopes.length === 0) {
    throw new Error('No valid backup files found');
  }

  const existing = await service.listTargets();
  const existingByName = new Map(existing.map((t) => [t.name, t.uuid]));
  const validateOpts = opts.validate ? { validate: true } : undefined;
  const results: RestoreResult[] = [];

  for (const env of envelopes) {
    const name = env.data.name as string;
    try {
      const existingUuid = existingByName.get(name);
      if (existingUuid && !opts.overwrite) {
        results.push({ name, action: 'skipped' });
        continue;
      }
      if (existingUuid) {
        await service.updateTarget(
          existingUuid,
          env.data as RedTeamTargetUpdateRequest,
          validateOpts,
        );
        results.push({ name, action: 'updated' });
      } else {
        await service.createTarget(
          env.data as RedTeamTargetCreateRequest,
          validateOpts,
        );
        results.push({ name, action: 'created' });
      }
    } catch (err) {
      results.push({
        name,
        action: 'failed',
        error: err instanceof Error ? err.message : String(err),
      });
    }
  }

  return results;
}

export function registerRestoreCommand(program: Command): void {
  const restore = program
    .command('restore')
    .description('Restore AIRS configuration from local backup files');

  restore
    .command('targets')
    .description('Restore red team targets from local JSON/YAML backup files')
    .option('--input-dir <path>', 'Directory containing backup files')
    .option('--file <path>', 'Single backup file to restore')
    .option('--overwrite', 'Update existing targets with same name (default: skip)')
    .option('--validate', 'Validate target connection before saving')
    .action(async (opts) => {
      try {
        renderBackupHeader();
        if (!opts.file && !opts.inputDir) {
          throw new Error('Specify --file <path> or --input-dir <path>');
        }
        const results = await restoreTargets({
          file: opts.file,
          inputDir: opts.inputDir,
          overwrite: opts.overwrite,
          validate: opts.validate,
        });
        renderRestoreSummary(results);
        const failed = results.filter((r) => r.action === 'failed').length;
        if (failed > 0) process.exit(1);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });
}
```

- [ ] **Step 2: Register in `src/cli/index.ts`**

Add import:

```typescript
import { registerRestoreCommand } from './commands/restore.js';
```

Add registration:

```typescript
registerRestoreCommand(program);
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `pnpm test -- tests/unit/cli/restore.spec.ts`
Expected: ALL PASS

- [ ] **Step 4: Run all tests**

Run: `pnpm test`
Expected: ALL PASS

- [ ] **Step 5: Type-check and lint**

Run: `pnpm tsc --noEmit && pnpm run lint:fix`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/cli/commands/restore.ts src/cli/index.ts
git commit -m "feat(backup): implement restore targets command"
```

---

### Task 11: Add library exports and changeset

**Files:**
- Modify: `src/index.ts:1-117` (add backup exports)
- Create: `.changeset/0005-backup-restore-targets.md`

- [ ] **Step 1: Add backup exports to `src/index.ts`**

Add a new section after the existing exports (before the Reports section):

```typescript
// ---------------------------------------------------------------------------
// Backup — export/import AIRS configuration to/from local files
// ---------------------------------------------------------------------------
export {
  readBackupDir,
  readBackupFile,
  resolveOutputDir,
  sanitizeFilename,
  writeBackupFile,
} from './backup/io.js';
export type {
  BackupEnvelope,
  BackupFormat,
  BackupResult,
  ResourceType,
  RestoreResult,
} from './backup/types.js';
```

- [ ] **Step 2: Create changeset**

```bash
cat > .changeset/0005-backup-restore-targets.md << 'EOF'
---
"@cdot65/prisma-airs-cli": minor
---

Add `airs backup targets` and `airs restore targets` commands for exporting/importing redteam target configurations to/from local JSON/YAML files. Supports single-target and bulk modes, with `--overwrite` for collision handling on restore.
EOF
```

- [ ] **Step 3: Type-check and run full test suite**

Run: `pnpm tsc --noEmit && pnpm test`
Expected: ALL PASS

- [ ] **Step 4: Commit**

```bash
git add src/index.ts .changeset/0005-backup-restore-targets.md
git commit -m "feat(backup): add library exports and changeset for backup/restore"
```

---

### Task 12: Update CLAUDE.md

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Add backup/restore to the directory structure section**

In the directory structure tree, add after the `audit/` block:

```
├── backup/
│   ├── types.ts           # BackupEnvelope<T>, BackupFormat, ResourceType, result types
│   ├── io.ts              # writeBackupFile, readBackupFile, readBackupDir, sanitizeFilename
│   └── index.ts           # Barrel exports
```

And in the `cli/commands/` section add:

```
│   ├── backup.ts          # airs backup targets command handler
│   ├── restore.ts         # airs restore targets command handler
```

And in the `cli/renderer/` section add:

```
│       ├── backup.ts      # Backup/restore summary rendering
```

And in the `tests/unit/` section add:

```
│   ├── backup/            # io.spec.ts
```

And in `tests/unit/cli/` add:

```
│   ├── cli/               # ..., backup.spec.ts, backup-renderer.spec.ts, restore.spec.ts
```

- [ ] **Step 2: Add backup/restore architecture section**

Add a new section after the Audit section:

```markdown
### Backup & Restore (`src/backup/`, `src/cli/commands/backup.ts`, `src/cli/commands/restore.ts`)
- `airs backup targets` — export all or single target to local JSON/YAML files
- `airs restore targets` — import targets from backup files, skip or overwrite existing
- Backup envelope: `{ version, resourceType, exportedAt, data }` — server fields (uuid/status/active) stripped
- Shared I/O utilities in `src/backup/io.ts` — extensible to future resource types (profiles, topics, prompt-sets)
- CLI: `airs backup targets [--output-dir <path>] [--format json|yaml] [--name <name>]`
- CLI: `airs restore targets [--input-dir <path>] [--file <path>] [--overwrite] [--validate]`
```

- [ ] **Step 3: Lint and type-check**

Run: `pnpm run lint:fix && pnpm tsc --noEmit`
Expected: PASS

- [ ] **Step 4: Run full test suite**

Run: `pnpm test`
Expected: ALL PASS

- [ ] **Step 5: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: add backup/restore to CLAUDE.md"
```
