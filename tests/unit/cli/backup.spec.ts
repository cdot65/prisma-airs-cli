import { afterEach, describe, expect, it, vi } from 'vitest';

const mockListTargets = vi.fn();
const mockGetTarget = vi.fn();
vi.mock('../../../src/airs/redteam.js', () => ({
  SdkRedTeamService: vi.fn().mockImplementation(() => ({
    listTargets: mockListTargets,
    getTarget: mockGetTarget,
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
    // background mapped to target_background
    expect(written.data.target_background).toEqual({ industry: 'tech' });
    expect(written.data.background).toBeUndefined();
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
