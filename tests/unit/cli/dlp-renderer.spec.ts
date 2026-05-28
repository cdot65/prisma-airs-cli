// biome-ignore-all lint/suspicious/noExplicitAny: test payloads use arbitrary SDK shapes
import { describe, expect, it, vi } from 'vitest';
import {
  dlpDictionaries,
  dlpFilteringProfiles,
  dlpPatterns,
  dlpProfiles,
  toKey,
} from '../../../src/cli/renderer/dlp.js';

describe('toKey (snake_case label transformer)', () => {
  it('multi-word labels become snake_case', () => {
    expect(toKey('Data Profile')).toBe('data_profile');
    expect(toKey('Scan Type')).toBe('scan_type');
    expect(toKey('File Based')).toBe('file_based');
    expect(toKey('File Types')).toBe('file_types');
    expect(toKey('Profile Type')).toBe('profile_type');
  });

  it('hyphens become underscores', () => {
    expect(toKey('Non-File Based')).toBe('non_file_based');
  });

  it('single-word labels lowercased unchanged', () => {
    expect(toKey('ID')).toBe('id');
    expect(toKey('Name')).toBe('name');
    expect(toKey('Version')).toBe('version');
  });

  it('empty string returns empty', () => {
    expect(toKey('')).toBe('');
  });
});

describe('dlp renderGet json keys are snake_case (regression #105)', () => {
  it('filtering-profiles get json uses snake_case keys, not mangled camelCase', () => {
    const spy = vi.spyOn(console, 'log').mockImplementation(() => {});
    dlpFilteringProfiles.renderGet(
      {
        id: 'a',
        name: 'p',
        type: 'custom',
        data_profile_id: 11995047,
        direction: 'c2s',
        log_severity: 'low',
        scan_type: 'include',
        file_based: true,
        non_file_based: false,
        version: 1,
        file_type: new Array(35).fill('txt'),
      } as any,
      'json',
    );
    const out = String(spy.mock.calls[0]?.[0]);
    expect(out).toContain('"data_profile"');
    expect(out).toContain('"scan_type"');
    expect(out).toContain('"file_based"');
    expect(out).toContain('"non_file_based"');
    expect(out).toContain('"file_types"');
    expect(out).not.toContain('datarofile');
    expect(out).not.toContain('scanype');
    expect(out).not.toContain('fileased');
    expect(out).not.toContain('nonileased');
    expect(out).not.toContain('fileypes');
    spy.mockRestore();
  });

  it('profiles get json uses profile_type, not profileype', () => {
    const spy = vi.spyOn(console, 'log').mockImplementation(() => {});
    dlpProfiles.renderGet(
      { id: 'dp', name: 'n', type: 'advanced', profile_type: 'predefined', version: 2 } as any,
      'json',
    );
    const out = String(spy.mock.calls[0]?.[0]);
    expect(out).toContain('"profile_type"');
    expect(out).not.toContain('profileype');
    spy.mockRestore();
  });
});

describe('dlpFilteringProfiles renderer', () => {
  it('renderList json emits curated items + page meta (not raw SDK envelope)', () => {
    const spy = vi.spyOn(console, 'log').mockImplementation(() => {});
    dlpFilteringProfiles.renderList(
      {
        content: [{ id: 'a', name: 'p', type: 'custom', is_parent_managed: false }],
        totalElements: 1,
        pageable: { pageNumber: 0, pageSize: 25 },
      } as any,
      'json',
    );
    const out = String(spy.mock.calls[0]?.[0]);
    expect(out).toContain('"items"');
    expect(out).toContain('"page"');
    expect(out).toContain('"total": 1');
    expect(out).not.toContain('is_parent_managed');
    spy.mockRestore();
  });

  it('renderGet json emits curated fields only', () => {
    const spy = vi.spyOn(console, 'log').mockImplementation(() => {});
    dlpFilteringProfiles.renderGet(
      { id: 'a', name: 'p', type: 'custom', tenant_id: 'X', is_parent_managed: false } as any,
      'json',
    );
    const out = String(spy.mock.calls[0]?.[0]);
    expect(out).toContain('"id": "a"');
    expect(out).toContain('"name": "p"');
    expect(out).not.toContain('tenant_id');
    expect(out).not.toContain('is_parent_managed');
    spy.mockRestore();
  });

  it('renderReplaced pretty emits confirmation line', () => {
    const spy = vi.spyOn(console, 'log').mockImplementation(() => {});
    dlpFilteringProfiles.renderReplaced({ id: 'a' } as any, 'pretty');
    expect(spy).toHaveBeenCalledWith(expect.stringContaining('replaced'));
    spy.mockRestore();
  });

  it('renderReplaced json emits curated ack object', () => {
    const spy = vi.spyOn(console, 'log').mockImplementation(() => {});
    dlpFilteringProfiles.renderReplaced(
      { id: 'a', name: 'p', version: 2, tenant_id: 'X' } as any,
      'json',
    );
    const out = String(spy.mock.calls[0]?.[0]);
    expect(out).toContain('"action": "replaced"');
    expect(out).toContain('"id": "a"');
    expect(out).toContain('"version": 2');
    expect(out).not.toContain('tenant_id');
    spy.mockRestore();
  });
});

describe('dlpPatterns renderer', () => {
  it('renderArchived prints "archived <id>"', () => {
    const spy = vi.spyOn(console, 'log').mockImplementation(() => {});
    dlpPatterns.renderArchived('p1');
    expect(spy).toHaveBeenCalledWith(expect.stringContaining('archived'));
    expect(spy).toHaveBeenCalledWith(expect.stringContaining('p1'));
    spy.mockRestore();
  });

  it('renderCreated/Patched/Replaced json emit curated ack objects', () => {
    const spy = vi.spyOn(console, 'log').mockImplementation(() => {});
    dlpPatterns.renderCreated({ id: 'p1', name: 'x', version: 1 } as any, 'json');
    dlpPatterns.renderPatched({ id: 'p1', name: 'x', version: 2 } as any, 'json');
    dlpPatterns.renderReplaced({ id: 'p1', name: 'x', version: 3 } as any, 'json');
    expect(spy.mock.calls.every((c) => String(c[0]).includes('"id": "p1"'))).toBe(true);
    expect(String(spy.mock.calls[0]?.[0])).toContain('"action": "created"');
    expect(String(spy.mock.calls[1]?.[0])).toContain('"action": "patched"');
    expect(String(spy.mock.calls[2]?.[0])).toContain('"action": "replaced"');
    spy.mockRestore();
  });
});

describe('dlpProfiles renderer', () => {
  it('renderList json emits curated items + page', () => {
    const spy = vi.spyOn(console, 'log').mockImplementation(() => {});
    dlpProfiles.renderList(
      {
        content: [{ id: 'dp', name: 'n', tenant_id: 'X' }],
        totalElements: 1,
        pageable: { pageNumber: 0 },
      } as any,
      'json',
    );
    const out = String(spy.mock.calls[0]?.[0]);
    expect(out).toContain('"id": "dp"');
    expect(out).toContain('"total": 1');
    expect(out).not.toContain('tenant_id');
    spy.mockRestore();
  });
});

describe('dlpDictionaries renderer', () => {
  it('renderDeleted prints "deleted <id>"', () => {
    const spy = vi.spyOn(console, 'log').mockImplementation(() => {});
    dlpDictionaries.renderDeleted('d1');
    expect(spy).toHaveBeenCalledWith(expect.stringContaining('deleted'));
    expect(spy).toHaveBeenCalledWith(expect.stringContaining('d1'));
    spy.mockRestore();
  });

  it('renderReplaced204Fallback', () => {
    const spy = vi.spyOn(console, 'log').mockImplementation(() => {});
    dlpDictionaries.renderReplaced204Fallback('d1');
    expect(spy).toHaveBeenCalledWith(expect.stringContaining('replaced'));
    expect(spy).toHaveBeenCalledWith(expect.stringContaining('d1'));
    expect(spy).toHaveBeenCalledWith(expect.stringContaining('state not echoed'));
    spy.mockRestore();
  });
});
