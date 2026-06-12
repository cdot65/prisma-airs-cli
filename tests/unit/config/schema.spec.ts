import { describe, expect, it } from 'vitest';
import { ConfigSchema, LlmProviderSchema } from '../../../src/config/schema.js';

describe('ConfigSchema', () => {
  it('returns Zod defaults on empty input', () => {
    const config = ConfigSchema.parse({});
    expect(config.llmProvider).toBe('claude-api');
    expect(config.googleCloudLocation).toBe('us-central1');
    expect(config.awsRegion).toBe('us-east-1');
    expect(config.scanConcurrency).toBe(5);
    expect(config.dataDir).toBe('~/.prisma-airs/runs');
  });

  it('coerces strings to numbers', () => {
    const config = ConfigSchema.parse({ scanConcurrency: '10' });
    expect(config.scanConcurrency).toBe(10);
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
      anthropicApiKey: 'sk-test',
      googleApiKey: 'gk-test',
      googleCloudProject: 'my-project',
    });
    expect(config.anthropicApiKey).toBe('sk-test');
    expect(config.googleApiKey).toBe('gk-test');
    expect(config.googleCloudProject).toBe('my-project');
  });
});

describe('LlmProviderSchema', () => {
  it('accepts all valid providers', () => {
    for (const p of [
      'claude-api',
      'claude-vertex',
      'claude-bedrock',
      'gemini-api',
      'gemini-vertex',
      'gemini-bedrock',
    ]) {
      expect(LlmProviderSchema.parse(p)).toBe(p);
    }
  });

  it('rejects invalid provider', () => {
    expect(() => LlmProviderSchema.parse('openai')).toThrow();
  });
});
