// biome-ignore-all lint/suspicious/noExplicitAny: test payloads use arbitrary SDK shapes
import { describe, expect, it, vi } from 'vitest';
import { dlpFilteringProfiles, dlpPatterns } from '../../../src/cli/renderer/dlp.js';

describe('dlpFilteringProfiles renderer', () => {
  it('renderList json emits content + totalElements', () => {
    const spy = vi.spyOn(console, 'log').mockImplementation(() => {});
    dlpFilteringProfiles.renderList(
      { content: [{ id: 'a', name: 'p' }], totalElements: 1, pageable: { pageNumber: 0 } } as any,
      'json',
    );
    expect(spy.mock.calls[0]?.[0]).toContain('"totalElements": 1');
    spy.mockRestore();
  });
  it('renderGet json round-trips', () => {
    const spy = vi.spyOn(console, 'log').mockImplementation(() => {});
    dlpFilteringProfiles.renderGet({ id: 'a' } as any, 'json');
    expect(spy.mock.calls[0]?.[0]).toContain('"id": "a"');
    spy.mockRestore();
  });
  it('renderReplaced emits confirmation', () => {
    const spy = vi.spyOn(console, 'log').mockImplementation(() => {});
    dlpFilteringProfiles.renderReplaced({ id: 'a' } as any, 'pretty');
    expect(spy).toHaveBeenCalledWith(expect.stringContaining('replaced'));
    spy.mockRestore();
  });
});

describe('dlpPatterns renderer', () => {
  it('renderArchived prints "archived <id>"', () => {
    const spy = vi.spyOn(console, 'log').mockImplementation(() => {});
    dlpPatterns.renderArchived('p1');
    expect(spy).toHaveBeenCalledWith(expect.stringContaining('archived p1'));
    spy.mockRestore();
  });
  it('renderCreated/Patched/Replaced emit json on demand', () => {
    const spy = vi.spyOn(console, 'log').mockImplementation(() => {});
    dlpPatterns.renderCreated({ id: 'p1' } as any, 'json');
    dlpPatterns.renderPatched({ id: 'p1', name: 'x' } as any, 'json');
    dlpPatterns.renderReplaced({ id: 'p1' } as any, 'json');
    expect(spy.mock.calls.every((c) => String(c[0]).includes('"id": "p1"'))).toBe(true);
    spy.mockRestore();
  });
});
