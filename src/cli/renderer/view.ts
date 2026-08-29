import { dump } from 'js-yaml';
import { formatOutput, type OutputFormat } from './common.js';
import { ui } from './ui.js';

export interface ColumnSpec<T> {
  key: string;
  label: string;
  get?: (item: T) => unknown;
}
export interface ResourceView<T> {
  name: string;
  columns: ColumnSpec<T>[];
  structured?: (item: T) => Record<string, unknown>;
  pretty: { list(items: T[]): void; detail(item: T): void };
}
export interface PageMeta {
  returned: number;
  total?: number;
  next?: number;
  all?: boolean;
}

function asRecord<T>(item: T): Record<string, unknown> {
  return item as Record<string, unknown>;
}
function project<T>(view: ResourceView<T>, item: T): Record<string, unknown> {
  const source = asRecord(item);
  return Object.fromEntries(
    view.columns.map((column) => [column.key, column.get ? column.get(item) : source[column.key]]),
  );
}
function renderPageStatus(page?: PageMeta): void {
  if (!page) return;
  if (page.all) ui.status(`Showing all ${page.total ?? page.returned}`);
  else if (page.total !== undefined)
    ui.status(
      `Showing ${page.returned} of ${page.total}${page.next !== undefined ? ` (next --offset ${page.next})` : ''}`,
    );
  else if (page.next !== undefined) ui.status(`Showing ${page.returned} (more available)`);
  else ui.status(`Showing ${page.returned}`);
}

export function emitList<T>(
  view: ResourceView<T>,
  items: T[],
  format: OutputFormat,
  opts: { page?: PageMeta } = {},
): void {
  if (format === 'pretty') {
    if (items.length === 0) ui.emptyList(view.name);
    else view.pretty.list(items);
  } else {
    const rows =
      format === 'json' || format === 'yaml'
        ? items.map((item) => view.structured?.(item) ?? asRecord(item))
        : items.map((item) => project(view, item));
    const rendered = formatOutput(rows, view.columns, format);
    if (rendered) console.log(rendered);
  }
  renderPageStatus(opts.page);
}

export function emitDetail<T>(view: ResourceView<T>, item: T, format: OutputFormat): void {
  if (format === 'pretty') {
    view.pretty.detail(item);
    return;
  }
  const structured = view.structured?.(item) ?? asRecord(item);
  if (format === 'json') console.log(JSON.stringify(structured, null, 2));
  else if (format === 'yaml')
    console.log(dump(structured, { noRefs: true, lineWidth: -1 }).trimEnd());
  else {
    const rows = Object.entries(structured).map(([key, value]) => ({
      key,
      value: value != null && typeof value === 'object' ? JSON.stringify(value) : value,
    }));
    console.log(
      formatOutput(
        rows,
        [
          { key: 'key', label: 'Key' },
          { key: 'value', label: 'Value' },
        ],
        format,
      ),
    );
  }
}
