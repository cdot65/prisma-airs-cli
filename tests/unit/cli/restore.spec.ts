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
    expect(failed?.error).toBe('API error');
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
