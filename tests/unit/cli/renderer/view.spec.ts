import { afterEach, describe, expect, it, vi } from 'vitest';
import { emitDetail, emitList, type ResourceView } from '../../../../src/cli/renderer/view.js';

interface RecordFixture extends Record<string, unknown> {
  id: string;
  name: string;
  note: string;
  nested: { enabled: boolean };
}

const item: RecordFixture = {
  id: 'one',
  name: 'A | B',
  note: 'line 1\nline 2: yes',
  nested: { enabled: true },
};

const view: ResourceView<RecordFixture> = {
  name: 'fixtures',
  columns: [
    { key: 'id', label: 'ID' },
    { key: 'name', label: 'Name' },
    { key: 'note', label: 'Note' },
  ],
  pretty: { list: vi.fn(), detail: vi.fn() },
};

describe('resource view emitters', () => {
  const log = vi.spyOn(console, 'log').mockImplementation(() => undefined);
  const error = vi.spyOn(console, 'error').mockImplementation(() => undefined);

  afterEach(() => {
    log.mockClear();
    error.mockClear();
    vi.mocked(view.pretty.list).mockClear();
    vi.mocked(view.pretty.detail).mockClear();
  });

  it('emits full list records as JSON and YAML', () => {
    emitList(view, [item], 'json');
    expect(JSON.parse(String(log.mock.calls[0][0]))).toEqual([item]);
    log.mockClear();
    emitList(view, [item], 'yaml');
    expect(String(log.mock.calls[0][0])).toContain('note: |-');
  });

  it('projects list columns for RFC 4180 CSV and Markdown', () => {
    emitList(view, [item], 'csv');
    expect(log).toHaveBeenCalledWith('ID,Name,Note\none,A | B,"line 1\nline 2: yes"');
    log.mockClear();
    emitList(view, [item], 'markdown');
    expect(log).toHaveBeenCalledWith(expect.stringContaining('A \\| B'));
  });

  it('renders detail structured formats from the full object', () => {
    emitDetail(view, item, 'json');
    expect(JSON.parse(String(log.mock.calls[0][0]))).toEqual(item);
    log.mockClear();
    emitDetail(view, item, 'csv');
    expect(log).toHaveBeenCalledWith(expect.stringContaining('nested,"{""enabled"":true}"'));
  });

  it('uses an optional structured normalization boundary for JSON and YAML', () => {
    const normalized = {
      ...view,
      structured: (record: RecordFixture) => ({ recordId: record.id }),
    };
    emitList(normalized, [item], 'json');
    expect(JSON.parse(String(log.mock.calls[0][0]))).toEqual([{ recordId: 'one' }]);
    log.mockClear();
    emitDetail(normalized, item, 'yaml');
    expect(log).toHaveBeenCalledWith('recordId: one');
  });

  it('delegates pretty output and reports page status on stderr', () => {
    emitList(view, [item], 'pretty', { page: { returned: 1, total: 2, next: 1 } });
    expect(view.pretty.list).toHaveBeenCalledWith([item]);
    expect(error).toHaveBeenCalledWith(expect.stringContaining('Showing 1 of 2'));
  });

  it('uses the canonical empty state for an empty pretty list', () => {
    emitList(view, [], 'pretty');
    expect(log).toHaveBeenCalledWith(expect.stringContaining('No fixtures found'));
    expect(view.pretty.list).not.toHaveBeenCalled();
  });
});
