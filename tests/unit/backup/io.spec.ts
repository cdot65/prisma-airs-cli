import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import {
  readBackupDir,
  readBackupFile,
  resolveOutputDir,
  sanitizeFilename,
  writeBackupFile,
} from '../../../src/backup/io.js';
import type { BackupEnvelope } from '../../../src/backup/types.js';

describe('sanitizeFilename', () => {
  it('lowercases and replaces spaces with hyphens', () => {
    expect(sanitizeFilename('My Target Name')).toBe('my-target-name');
  });

  it('replaces non-alphanumeric chars with hyphens', () => {
    expect(sanitizeFilename('test/target@v2!')).toBe('test-target-v2');
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

describe('readBackupDir', () => {
  const envelope: BackupEnvelope<{ name: string }> = {
    version: '1',
    resourceType: 'redteam-target',
    exportedAt: '2026-04-11T00:00:00.000Z',
    data: { name: 'test-target' },
  };

  it('reads all matching files from directory', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'backup-test-'));
    const env1: BackupEnvelope<{ name: string }> = {
      ...envelope,
      data: { name: 'target-a' },
    };
    const env2: BackupEnvelope<{ name: string }> = {
      ...envelope,
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
    writeBackupFile(dir, 'target-a', envelope, 'json');
    fs.writeFileSync(
      path.join(dir, 'other.json'),
      JSON.stringify({ version: '1', resourceType: 'other-thing', exportedAt: '', data: {} }),
    );
    const results = readBackupDir<{ name: string }>(dir, 'redteam-target');
    expect(results).toHaveLength(1);
    expect(results[0].data.name).toBe('test-target');
    fs.rmSync(dir, { recursive: true });
  });
});
