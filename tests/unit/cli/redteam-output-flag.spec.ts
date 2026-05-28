import { load as yamlLoad } from 'js-yaml';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

let output: string[];
const originalLog = console.log;

beforeEach(() => {
  output = [];
  console.log = (...args: unknown[]) => output.push(args.join(' '));
});

afterEach(() => {
  console.log = originalLog;
});

const samplePromptSet = {
  uuid: 'ps-uuid-001',
  name: 'sample-set',
  active: true,
  archive: false,
  description: 'sample description',
  createdAt: '2026-01-02T03:04:05Z',
  updatedAt: '2026-01-02T03:04:05Z',
};

const sampleVersionInfo = {
  uuid: 'ps-uuid-001',
  version: 7,
  stats: { total: 42, active: 40, inactive: 2 },
};

const sampleValues = { name: 'persona', values: ['pirate', 'doctor'] };
const sampleNames = ['persona', 'severity'];

const sampleTarget = {
  uuid: 'tg-uuid-001',
  name: 'litellm-mistral-7b',
  status: 'INACTIVE',
  targetType: 'APPLICATION',
  active: false,
  connectionParams: {
    api_endpoint: 'http://litellm.example.local:4000/v1/chat/completions',
    request_headers: { 'Content-Type': 'application/json', apikey: 'sk-xxx' },
    request_json: {
      model: 'mistral-7b',
      messages: [{ role: 'user', content: '{INPUT}' }],
    },
    response_json: { choices: [{ message: { content: '' } }] },
    response_key: 'choices.0.message.content',
  },
  background: { industry: 'Generic', use_case: 'Chatbot' },
  metadata: { rate_limit: 50, multi_turn: false },
};

describe('renderPromptSetDetail --output', () => {
  it('emits JSON with combined detail + versionInfo when format=json', async () => {
    const { renderPromptSetDetail } = await import('../../../src/cli/renderer/redteam.js');
    renderPromptSetDetail(samplePromptSet, 'json', sampleVersionInfo);
    const parsed = JSON.parse(output.join('\n'));
    expect(parsed.uuid).toBe('ps-uuid-001');
    expect(parsed.name).toBe('sample-set');
    expect(parsed.versionInfo.version).toBe(7);
    expect(parsed.versionInfo.stats.total).toBe(42);
  });

  it('emits YAML with combined detail + versionInfo when format=yaml', async () => {
    const { renderPromptSetDetail } = await import('../../../src/cli/renderer/redteam.js');
    renderPromptSetDetail(samplePromptSet, 'yaml', sampleVersionInfo);
    const parsed = yamlLoad(output.join('\n')) as Record<string, unknown>;
    expect(parsed.uuid).toBe('ps-uuid-001');
    expect(parsed.name).toBe('sample-set');
    expect((parsed.versionInfo as { version: number }).version).toBe(7);
  });

  it('still renders pretty form when format=pretty', async () => {
    const { renderPromptSetDetail } = await import('../../../src/cli/renderer/redteam.js');
    renderPromptSetDetail(samplePromptSet, 'pretty');
    const text = output.join('\n');
    expect(text).toContain('Prompt Set Detail');
    expect(text).toContain('sample-set');
  });
});

describe('renderPropertyValues --output', () => {
  it('emits JSON object with name + values[] when format=json', async () => {
    const { renderPropertyValues } = await import('../../../src/cli/renderer/redteam.js');
    renderPropertyValues(sampleValues, 'json');
    const parsed = JSON.parse(output.join('\n'));
    expect(parsed).toEqual(sampleValues);
    expect(Array.isArray(parsed.values)).toBe(true);
  });

  it('emits YAML when format=yaml', async () => {
    const { renderPropertyValues } = await import('../../../src/cli/renderer/redteam.js');
    renderPropertyValues(sampleValues, 'yaml');
    const parsed = yamlLoad(output.join('\n'));
    expect(parsed).toEqual(sampleValues);
  });

  it('renders pretty form with property name + bulleted values', async () => {
    const { renderPropertyValues } = await import('../../../src/cli/renderer/redteam.js');
    renderPropertyValues(sampleValues, 'pretty');
    const text = output.join('\n');
    expect(text).toContain('Property Values');
    expect(text).toContain('persona');
    expect(text).toContain('pirate');
    expect(text).toContain('doctor');
  });

  it('prints empty-state message when values list is empty in pretty mode', async () => {
    const { renderPropertyValues } = await import('../../../src/cli/renderer/redteam.js');
    renderPropertyValues({ name: 'persona', values: [] }, 'pretty');
    expect(output.join('\n')).toContain('No property values');
  });
});

describe('renderPropertyNames --output', () => {
  it('emits JSON string array when format=json', async () => {
    const { renderPropertyNames } = await import('../../../src/cli/renderer/redteam.js');
    renderPropertyNames(sampleNames, 'json');
    const parsed = JSON.parse(output.join('\n'));
    expect(parsed).toEqual(sampleNames);
  });

  it('emits YAML string list when format=yaml', async () => {
    const { renderPropertyNames } = await import('../../../src/cli/renderer/redteam.js');
    renderPropertyNames(sampleNames, 'yaml');
    const parsed = yamlLoad(output.join('\n'));
    expect(parsed).toEqual(sampleNames);
  });

  it('renders pretty form with bulleted names', async () => {
    const { renderPropertyNames } = await import('../../../src/cli/renderer/redteam.js');
    renderPropertyNames(sampleNames, 'pretty');
    const text = output.join('\n');
    expect(text).toContain('Property Names');
    expect(text).toContain('persona');
    expect(text).toContain('severity');
  });

  it('prints empty-state message for empty array in pretty mode', async () => {
    const { renderPropertyNames } = await import('../../../src/cli/renderer/redteam.js');
    renderPropertyNames([], 'pretty');
    expect(output.join('\n')).toContain('No property names');
  });
});

describe('renderTargetDetail --output', () => {
  it('emits JSON with full target payload when format=json', async () => {
    const { renderTargetDetail } = await import('../../../src/cli/renderer/redteam.js');
    renderTargetDetail(sampleTarget, 'json');
    const parsed = JSON.parse(output.join('\n'));
    expect(parsed.uuid).toBe('tg-uuid-001');
    expect(parsed.name).toBe('litellm-mistral-7b');
    expect(parsed.connectionParams.request_headers).toEqual({
      'Content-Type': 'application/json',
      apikey: 'sk-xxx',
    });
    expect(parsed.connectionParams.request_json.model).toBe('mistral-7b');
  });

  it('emits YAML with full target payload when format=yaml', async () => {
    const { renderTargetDetail } = await import('../../../src/cli/renderer/redteam.js');
    renderTargetDetail(sampleTarget, 'yaml');
    const parsed = yamlLoad(output.join('\n')) as Record<string, unknown>;
    expect(parsed.uuid).toBe('tg-uuid-001');
    const conn = parsed.connectionParams as Record<string, unknown>;
    expect((conn.request_headers as Record<string, string>).apikey).toBe('sk-xxx');
  });

  it('renders nested connection_params as indented JSON in pretty mode (not [object Object])', async () => {
    const { renderTargetDetail } = await import('../../../src/cli/renderer/redteam.js');
    renderTargetDetail(sampleTarget, 'pretty');
    const text = output.join('\n');
    expect(text).not.toContain('[object Object]');
    expect(text).toContain('request_headers');
    expect(text).toContain('apikey');
    expect(text).toContain('sk-xxx');
    expect(text).toContain('mistral-7b');
  });

  it('defaults to pretty when no format is passed', async () => {
    const { renderTargetDetail } = await import('../../../src/cli/renderer/redteam.js');
    renderTargetDetail(sampleTarget);
    const text = output.join('\n');
    expect(text).toContain('Target Detail');
    expect(text).not.toContain('[object Object]');
  });

  it('renders nested background/metadata objects without [object Object]', async () => {
    const { renderTargetDetail } = await import('../../../src/cli/renderer/redteam.js');
    renderTargetDetail(
      {
        ...sampleTarget,
        background: { industry: 'Generic', nested: { foo: 'bar' } },
        metadata: { tags: { a: 1, b: 2 } },
      },
      'pretty',
    );
    const text = output.join('\n');
    expect(text).not.toContain('[object Object]');
    expect(text).toContain('foo');
    expect(text).toContain('bar');
  });
});
