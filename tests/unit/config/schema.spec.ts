import { describe, expect, it } from 'vitest';
import { ConfigSchema } from '../../../src/config/schema.js';

describe('ConfigSchema', () => {
  it('returns Zod defaults on empty input', () => {
    const config = ConfigSchema.parse({});
    expect(config.scanConcurrency).toBe(5);
    expect(config.dataDir).toBe('~/.prisma-airs/runs');
  });

  it('coerces strings to numbers', () => {
    const config = ConfigSchema.parse({ scanConcurrency: '10' });
    expect(config.scanConcurrency).toBe(10);
  });

  it('accepts every output format and rejects unknown formats', () => {
    for (const defaultOutput of ['pretty', 'table', 'markdown', 'csv', 'json', 'yaml']) {
      expect(ConfigSchema.parse({ defaultOutput }).defaultOutput).toBe(defaultOutput);
    }
    expect(() => ConfigSchema.parse({ defaultOutput: 'xml' })).toThrow();
  });

  it('rejects scanConcurrency below 1', () => {
    expect(() => ConfigSchema.parse({ scanConcurrency: 0 })).toThrow();
  });

  it('rejects scanConcurrency above 20', () => {
    expect(() => ConfigSchema.parse({ scanConcurrency: 21 })).toThrow();
  });

  it('preserves scanner and shared management override fields when provided', () => {
    const config = ConfigSchema.parse({
      airsApiToken: 'tok-1',
      airsApiEndpoint: 'https://airs.example.com',
      mgmtEndpoint: 'https://mgmt.example.com',
      mgmtTokenEndpoint: 'https://token.example.com',
      mgmtDashboardEndpoint: 'https://dash.example.com/aisec',
    });
    expect(config.airsApiToken).toBe('tok-1');
    expect(config.airsApiEndpoint).toBe('https://airs.example.com');
    expect(config.mgmtEndpoint).toBe('https://mgmt.example.com');
    expect(config.mgmtTokenEndpoint).toBe('https://token.example.com');
    expect(config.mgmtDashboardEndpoint).toBe('https://dash.example.com/aisec');
  });

  it('drops retired per-product token endpoints but keeps base-URL overrides', () => {
    const config = ConfigSchema.parse({
      redTeamDataEndpoint: 'https://rt-data.example.com',
      redTeamTokenEndpoint: 'https://rt-token.example.com',
      modelSecTokenEndpoint: 'https://ms-token.example.com',
      agentGuardTokenEndpoint: 'https://ag-token.example.com',
      aiGwTokenEndpoint: 'https://gw-token.example.com',
      dlpEndpoint: 'https://dlp.example.com',
    }) as Record<string, unknown>;
    expect(config.redTeamDataEndpoint).toBe('https://rt-data.example.com');
    expect(config.dlpEndpoint).toBe('https://dlp.example.com');
    for (const key of Object.keys(config)) expect(key).not.toMatch(/TokenEndpoint$/);
  });

  it('defaults endpoint/auth override fields to undefined', () => {
    const config = ConfigSchema.parse({});
    expect(config.airsApiToken).toBeUndefined();
    expect(config.airsApiEndpoint).toBeUndefined();
    expect(config.airsNumRetries).toBeUndefined();
  });

  it('coerces airsNumRetries strings to numbers', () => {
    const config = ConfigSchema.parse({ airsNumRetries: '3' });
    expect(config.airsNumRetries).toBe(3);
  });

  it('rejects airsNumRetries below 0', () => {
    expect(() => ConfigSchema.parse({ airsNumRetries: -1 })).toThrow();
  });

  it('rejects airsNumRetries above 5', () => {
    expect(() => ConfigSchema.parse({ airsNumRetries: 6 })).toThrow();
  });

  it('preserves optional string fields when provided', () => {
    const config = ConfigSchema.parse({
      airsApiKey: 'sk-test',
      mgmtClientId: 'client-1',
      mgmtTsgId: 'tsg-1',
    });
    expect(config.airsApiKey).toBe('sk-test');
    expect(config.mgmtClientId).toBe('client-1');
    expect(config.mgmtTsgId).toBe('tsg-1');
  });

  it('keeps the AI Gateway inference key fields (workspace key, not SCM OAuth)', () => {
    const config = ConfigSchema.parse({
      aiGwInferenceEndpoint: 'https://gw.example.com/v1',
      aiGwInferenceApiKey: 'runtime-key',
    });
    expect(config.aiGwInferenceEndpoint).toBe('https://gw.example.com/v1');
    expect(config.aiGwInferenceApiKey).toBe('runtime-key');
  });
});
