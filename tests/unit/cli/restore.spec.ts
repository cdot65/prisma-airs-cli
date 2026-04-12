import { afterEach, describe, expect, it, vi } from 'vitest';

const mockListTargets = vi.fn();
const mockGetTarget = vi.fn();
const mockCreateTarget = vi.fn();
const mockUpdateTarget = vi.fn();
vi.mock('../../../src/airs/redteam.js', () => ({
  SdkRedTeamService: vi.fn().mockImplementation(() => ({
    listTargets: mockListTargets,
    getTarget: mockGetTarget,
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
import { prepareTargetPayload, restoreTargets } from '../../../src/cli/commands/restore.js';

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
    const payload = mockCreateTarget.mock.calls[0][0];
    expect(payload.name).toBe('Target A');
    expect(payload.connection_type).toBe('CUSTOM');
    expect(payload.api_endpoint_type).toBe('PUBLIC');
    expect(payload.response_mode).toBe('REST');
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
    mockGetTarget.mockResolvedValue({
      uuid: 'u1',
      name: 'Target A',
      targetType: 'OPENAI',
      connectionType: 'CUSTOM',
      apiEndpointType: 'DIRECT',
      responseMode: 'REST',
    });
    mockUpdateTarget.mockResolvedValue({ uuid: 'u1', name: 'Target A' });

    const results = await restoreTargets({
      file: path.join(dir, 'target-a.json'),
      overwrite: true,
    });

    expect(results).toHaveLength(1);
    expect(results[0].action).toBe('updated');
    expect(mockGetTarget).toHaveBeenCalledWith('u1');
    const payload = mockUpdateTarget.mock.calls[0][1];
    // Routing fields merged from existing target
    expect(payload.target_type).toBe('OPENAI');
    expect(payload.connection_type).toBe('CUSTOM');
    expect(payload.api_endpoint_type).toBe('DIRECT');
    expect(payload.response_mode).toBe('REST');
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

    const payload = mockCreateTarget.mock.calls[0][0];
    expect(payload.name).toBe('Target A');
    expect(payload.connection_type).toBe('CUSTOM');
    expect(mockCreateTarget).toHaveBeenCalledWith(payload, { validate: true });
    fs.rmSync(dir, { recursive: true });
  });

  it('strips server-derived fields and normalizes legacy names on create', async () => {
    const legacyEnvelope: BackupEnvelope<Record<string, unknown>> = {
      version: '1',
      resourceType: 'redteam-target',
      exportedAt: '2026-04-11T00:00:00.000Z',
      data: {
        name: 'Legacy Target',
        target_type: 'REST',
        connection_params: { api_endpoint: 'https://example.com' },
        background: { industry: 'tech' },
        metadata: { rate_limit: 10 },
        uuid: 'server-uuid',
        status: 'ACTIVE',
        active: true,
      },
    };
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'restore-'));
    writeBackupFile(dir, 'legacy', legacyEnvelope, 'json');
    mockListTargets.mockResolvedValue([]);
    mockCreateTarget.mockResolvedValue({ uuid: 'new-uuid', name: 'Legacy Target' });

    const results = await restoreTargets({ file: path.join(dir, 'legacy.json') });

    expect(results[0].action).toBe('created');
    const payload = mockCreateTarget.mock.calls[0][0];
    expect(payload.name).toBe('Legacy Target');
    expect(payload.target_background).toEqual({ industry: 'tech' });
    expect(payload.target_metadata).toEqual({ rate_limit: 10 });
    expect(payload).not.toHaveProperty('background');
    expect(payload).not.toHaveProperty('metadata');
    expect(payload).not.toHaveProperty('uuid');
    expect(payload).not.toHaveProperty('status');
    expect(payload).not.toHaveProperty('active');
    fs.rmSync(dir, { recursive: true });
  });
});

describe('prepareTargetPayload', () => {
  it('strips server-derived fields', () => {
    const result = prepareTargetPayload({
      name: 'test',
      target_type: 'REST',
      uuid: 'abc',
      tsg_id: '123',
      status: 'ACTIVE',
      active: true,
      validated: true,
      version: 123,
      secret_version: '1',
      auth_type: 'OAUTH2',
      auth_config: { token_url: 'https://example.com' },
    });
    expect(result).toEqual({
      name: 'test',
      target_type: 'REST',
      auth_type: 'OAUTH2',
      auth_config: { token_url: 'https://example.com' },
    });
  });

  it('renames legacy background → target_background', () => {
    const result = prepareTargetPayload({
      name: 'test',
      background: { industry: 'tech' },
    });
    expect(result.target_background).toEqual({ industry: 'tech' });
    expect(result).not.toHaveProperty('background');
  });

  it('renames legacy metadata → target_metadata', () => {
    const result = prepareTargetPayload({
      name: 'test',
      metadata: { rate_limit: 5 },
    });
    expect(result.target_metadata).toEqual({ rate_limit: 5 });
    expect(result).not.toHaveProperty('metadata');
  });

  it('preserves target_background over legacy background', () => {
    const result = prepareTargetPayload({
      name: 'test',
      background: { industry: 'old' },
      target_background: { industry: 'new' },
    });
    expect(result.target_background).toEqual({ industry: 'new' });
    expect(result).not.toHaveProperty('background');
  });
});
