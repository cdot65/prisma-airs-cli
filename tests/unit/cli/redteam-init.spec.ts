import { describe, expect, it } from 'vitest';

/** Valid provider names for target templates. */
const VALID_PROVIDERS = [
  'OPENAI',
  'HUGGING_FACE',
  'DATABRICKS',
  'BEDROCK',
  'REST',
  'STREAMING',
  'WEBSOCKET',
];

/**
 * Build a target config scaffold from a provider template.
 * Extracted as a pure function so it can be tested without CLI/service wiring.
 */
function buildTargetScaffold(
  provider: string,
  templates: Record<string, unknown>,
): Record<string, unknown> {
  const key = provider.toUpperCase();
  if (!VALID_PROVIDERS.includes(key)) {
    throw new Error(
      `Unknown provider "${provider}". Valid providers: ${VALID_PROVIDERS.join(', ')}`,
    );
  }
  return {
    name: '',
    target_type: key,
    connection_params: templates[key] ?? {},
    background: {},
    additional_context: {},
    metadata: {},
  };
}

describe('buildTargetScaffold', () => {
  const mockTemplates: Record<string, unknown> = {
    OPENAI: { api_key: '', model: 'gpt-4', endpoint: 'https://api.openai.com/v1' },
    HUGGING_FACE: { token: '', model_id: '' },
    DATABRICKS: { host: '', token: '' },
    BEDROCK: { region: '', model_id: '' },
    REST: { url: '', method: 'POST' },
    STREAMING: { url: '', method: 'POST' },
    WEBSOCKET: { url: '' },
  };

  it('builds correct scaffold for valid provider', () => {
    const result = buildTargetScaffold('openai', mockTemplates);
    expect(result).toEqual({
      name: '',
      target_type: 'OPENAI',
      connection_params: mockTemplates.OPENAI,
      background: {},
      additional_context: {},
      metadata: {},
    });
  });

  it('handles case-insensitive provider input', () => {
    const result = buildTargetScaffold('Hugging_Face', mockTemplates);
    expect(result.target_type).toBe('HUGGING_FACE');
  });

  it('throws on invalid provider with list of valid providers', () => {
    expect(() => buildTargetScaffold('azure', mockTemplates)).toThrow(
      'Unknown provider "azure". Valid providers: OPENAI, HUGGING_FACE, DATABRICKS, BEDROCK, REST, STREAMING, WEBSOCKET',
    );
  });
});
