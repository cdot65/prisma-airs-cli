import { describe, expect, it } from 'vitest';
import { buildTargetScaffold } from '../../../src/cli/commands/redteam.js';

describe('buildTargetScaffold', () => {
  const mockTemplates: Record<string, unknown> = {
    OPENAI: { api_key: '', model: 'gpt-4', endpoint: 'https://api.openai.com/v1' },
    HUGGING_FACE: {
      url: 'https://router.huggingface.co/v1/chat/completions',
      request_json: {},
      response_json: {},
    },
    DATABRICKS: { host: '', token: '' },
    BEDROCK: { region: '', model_id: '' },
    REST: { url: 'https://api.example.com', method: 'POST' },
    STREAMING: { url: '', method: 'POST' },
  };

  it('native provider scaffold has correct top-level fields', () => {
    const result = buildTargetScaffold('openai', mockTemplates);
    expect(result.name).toBe('');
    expect(result.target_type).toBe('APPLICATION');
    expect(result.connection_type).toBe('OPENAI');
    expect(result.api_endpoint_type).toBe('PUBLIC');
    expect(result.response_mode).toBe('REST');
    expect(result.auth_type).toBe('HEADERS');
    // native providers use target_connection_config inside connection_params
    expect((result.connection_params as Record<string, unknown>).target_connection_config).toEqual(
      mockTemplates.OPENAI,
    );
  });

  it('REST provider scaffold uses api_endpoint and response_key (not url)', () => {
    const result = buildTargetScaffold('REST', mockTemplates);
    const cp = result.connection_params as Record<string, unknown>;
    expect(result.connection_type).toBe('CUSTOM');
    expect(result.response_mode).toBe('REST');
    expect(cp.api_endpoint).toBe('https://api.example.com');
    expect(cp.response_key).toBe('choices.0.message.content');
    expect(cp).not.toHaveProperty('url');
  });

  it('HUGGING_FACE scaffold inherits url as api_endpoint', () => {
    const result = buildTargetScaffold('HUGGING_FACE', mockTemplates);
    const cp = result.connection_params as Record<string, unknown>;
    expect(cp.api_endpoint).toBe('https://router.huggingface.co/v1/chat/completions');
    expect(cp.response_key).toBe('choices.0.message.content');
  });

  it('CUSTOM_TARGET_ADAPTER scaffold targets AGENT type via NETWORK_BROKER', () => {
    const result = buildTargetScaffold('CUSTOM_TARGET_ADAPTER', mockTemplates);
    expect(result.target_type).toBe('AGENT');
    expect(result.connection_type).toBe('CUSTOM_TARGET_ADAPTER');
    expect(result.api_endpoint_type).toBe('NETWORK_BROKER');
    expect(result).toHaveProperty('adapter_uuid');
    expect(Array.isArray(result.adapter_variable_overrides)).toBe(true);
  });

  it('handles case-insensitive provider input', () => {
    const result = buildTargetScaffold('Hugging_Face', mockTemplates);
    expect(result.target_type).toBe('APPLICATION');
  });

  it('throws on invalid provider with list of valid providers', () => {
    expect(() => buildTargetScaffold('azure', mockTemplates)).toThrow(
      'Unknown provider "azure". Valid providers: OPENAI, HUGGING_FACE, DATABRICKS, BEDROCK, REST, STREAMING, WEBSOCKET, CUSTOM_TARGET_ADAPTER',
    );
  });
});
