// biome-ignore-all lint/suspicious/noExplicitAny: test payloads use arbitrary SDK shapes
import { describe, expect, it, vi } from 'vitest';
import { renderGroupDetail } from '../../../src/cli/renderer/modelsecurity.js';

const group: any = {
  uuid: '00000000-0000-0000-0000-000000000001',
  name: 'demo-group',
  description: 'a demo group',
  sourceType: 'HUGGING_FACE',
  state: 'ACTIVE',
  createdAt: '2026-06-05T00:00:00Z',
  updatedAt: '2026-06-05T00:00:00Z',
};

describe('renderGroupDetail --output (#238)', () => {
  it('json: emits the full group object as parseable JSON', () => {
    const spy = vi.spyOn(console, 'log').mockImplementation(() => {});
    renderGroupDetail(group, 'json');
    const out = spy.mock.calls.map((c) => c[0]).join('\n');
    spy.mockRestore();
    const parsed = JSON.parse(out);
    expect(parsed.uuid).toBe(group.uuid);
    expect(parsed.name).toBe('demo-group');
    expect(parsed.state).toBe('ACTIVE');
  });

  it('yaml: emits the group as YAML', () => {
    const spy = vi.spyOn(console, 'log').mockImplementation(() => {});
    renderGroupDetail(group, 'yaml');
    const out = spy.mock.calls.map((c) => c[0]).join('\n');
    spy.mockRestore();
    expect(out).toMatch(/uuid:\s*00000000-0000-0000-0000-000000000001/);
    expect(out).toMatch(/name:\s*demo-group/);
  });

  it('pretty (default): unchanged human layout', () => {
    const spy = vi.spyOn(console, 'log').mockImplementation(() => {});
    renderGroupDetail(group);
    const out = spy.mock.calls.map((c) => c[0]).join('\n');
    spy.mockRestore();
    expect(out).toContain('Security Group Detail');
    expect(out).toContain('demo-group');
  });
});
