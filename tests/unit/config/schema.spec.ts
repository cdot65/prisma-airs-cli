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
