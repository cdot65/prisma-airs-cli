import { afterEach, describe, expect, it } from 'vitest';
import type { BackupResult, RestoreResult } from '../../../src/backup/types.js';

let output: string[];
const originalLog = console.log;

describe('renderBackupSummary', () => {
  afterEach(() => {
    output = [];
    console.log = originalLog;
  });

  it('renders backup results with count and directory', async () => {
    output = [];
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
    output = [];
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

describe('renderRestoreSummary', () => {
  afterEach(() => {
    output = [];
    console.log = originalLog;
  });

  it('renders restore results with action totals', async () => {
    output = [];
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
    output = [];
    console.log = (...args: unknown[]) => output.push(args.join(' '));
    const { renderRestoreSummary } = await import('../../../src/cli/renderer/backup.js');
    const results: RestoreResult[] = [{ name: 'target-a', action: 'failed', error: 'API error' }];
    renderRestoreSummary(results);
    const text = output.join('\n');
    expect(text).toContain('target-a');
    expect(text).toContain('failed');
    expect(text).toContain('API error');
  });
});
