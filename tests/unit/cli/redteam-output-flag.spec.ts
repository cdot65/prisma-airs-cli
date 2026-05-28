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

const sampleValues = [
  { name: 'persona', value: 'pirate' },
  { name: 'persona', value: 'doctor' },
];

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
  it('emits JSON array when format=json', async () => {
    const { renderPropertyValues } = await import('../../../src/cli/renderer/redteam.js');
    renderPropertyValues(sampleValues, 'json');
    const parsed = JSON.parse(output.join('\n'));
    expect(Array.isArray(parsed)).toBe(true);
    expect(parsed).toEqual(sampleValues);
  });

  it('emits YAML when format=yaml', async () => {
    const { renderPropertyValues } = await import('../../../src/cli/renderer/redteam.js');
    renderPropertyValues(sampleValues, 'yaml');
    const parsed = yamlLoad(output.join('\n'));
    expect(parsed).toEqual(sampleValues);
  });

  it('still renders pretty form when format=pretty', async () => {
    const { renderPropertyValues } = await import('../../../src/cli/renderer/redteam.js');
    renderPropertyValues(sampleValues, 'pretty');
    const text = output.join('\n');
    expect(text).toContain('Property Values');
    expect(text).toContain('persona');
  });

  it('prints empty-state message for empty list in pretty mode', async () => {
    const { renderPropertyValues } = await import('../../../src/cli/renderer/redteam.js');
    renderPropertyValues([], 'pretty');
    expect(output.join('\n')).toContain('No property values');
  });
});
