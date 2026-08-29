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

  it('preserves endpoint/auth override fields when provided', () => {
    const config = ConfigSchema.parse({
      airsApiToken: 'tok-1',
      airsApiEndpoint: 'https://airs.example.com',
      redTeamDataEndpoint: 'https://rt-data.example.com',
      redTeamMgmtEndpoint: 'https://rt-mgmt.example.com',
      redTeamTokenEndpoint: 'https://rt-token.example.com',
      modelSecDataEndpoint: 'https://ms-data.example.com',
      modelSecMgmtEndpoint: 'https://ms-mgmt.example.com',
      modelSecTokenEndpoint: 'https://ms-token.example.com',
    });
    expect(config.airsApiToken).toBe('tok-1');
    expect(config.airsApiEndpoint).toBe('https://airs.example.com');
    expect(config.redTeamDataEndpoint).toBe('https://rt-data.example.com');
    expect(config.redTeamMgmtEndpoint).toBe('https://rt-mgmt.example.com');
    expect(config.redTeamTokenEndpoint).toBe('https://rt-token.example.com');
    expect(config.modelSecDataEndpoint).toBe('https://ms-data.example.com');
    expect(config.modelSecMgmtEndpoint).toBe('https://ms-mgmt.example.com');
    expect(config.modelSecTokenEndpoint).toBe('https://ms-token.example.com');
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

  it('preserves AI Gateway endpoint fields when provided', () => {
    const config = ConfigSchema.parse({
      aiGwDataEndpoint: 'https://gw-data.example.com/ai_gw/v2',
      aiGwAdminEndpoint: 'https://gw-admin.example.com/ai_gw/admin/v2',
      aiGwTokenEndpoint: 'https://gw-token.example.com',
    });
    expect(config.aiGwDataEndpoint).toBe('https://gw-data.example.com/ai_gw/v2');
    expect(config.aiGwAdminEndpoint).toBe('https://gw-admin.example.com/ai_gw/admin/v2');
    expect(config.aiGwTokenEndpoint).toBe('https://gw-token.example.com');
  });

  it('defaults AI Gateway endpoint fields to undefined (SDK falls back to env/defaults)', () => {
    const config = ConfigSchema.parse({});
    expect(config.aiGwDataEndpoint).toBeUndefined();
    expect(config.aiGwAdminEndpoint).toBeUndefined();
    expect(config.aiGwTokenEndpoint).toBeUndefined();
  });
});
