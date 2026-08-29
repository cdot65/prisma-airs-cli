import chalk from 'chalk';
import { dump as yamlDump } from 'js-yaml';
import { formatOutput, type OutputFormat } from './common.js';
import { ui } from './ui.js';
import {
  emitDetail as emitViewDetail,
  emitList as emitViewList,
  type ResourceView,
} from './view.js';

// biome-ignore lint/suspicious/noExplicitAny: renderer accepts arbitrary SDK payloads
type Any = any;

/** Status → inline color (value coloring within composed list lines). */
function statusColor(status: string | undefined): string {
  switch (status) {
    case 'active':
      return chalk.green(status);
    case 'deleted':
    case 'disabled':
      return chalk.yellow(status);
    default:
      return status ? chalk.dim(status) : chalk.dim('—');
  }
}

function ts(ms: number | undefined): string | undefined {
  if (!ms) return undefined;
  try {
    return new Date(ms).toISOString();
  } catch {
    return undefined;
  }
}

function emitStructured(payload: unknown, fmt: OutputFormat): void {
  if (fmt === 'json') {
    console.log(JSON.stringify(payload, null, 2));
    return;
  }
  if (fmt === 'yaml') {
    console.log(yamlDump(payload));
    return;
  }
  console.log(JSON.stringify(payload, null, 2));
}

function pageMeta(page: Any, returned: number): Record<string, unknown> {
  const p = page?.pageable ?? {};
  return {
    number: p?.pageNumber ?? p?.page_number ?? 0,
    size: p?.pageSize ?? p?.page_size ?? returned,
    total: page?.totalElements ?? page?.total_elements,
    returned,
  };
}

function camelKey(key: string): string {
  return key.replace(/[_-]([a-z0-9])/g, (_, char: string) => char.toUpperCase());
}

function camelize(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(camelize);
  if (value && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value as Record<string, unknown>).map(([key, child]) => [
        camelKey(key),
        camelize(child),
      ]),
    );
  }
  return value;
}

function emitList(
  page: Any,
  fmt: OutputFormat,
  header: string,
  toRow: (item: Any) => Record<string, unknown>,
  columns: { key: string; label: string }[],
  prettyLine: (item: Any) => string,
): void {
  const content: Any[] = Array.isArray(page?.content) ? page.content : [];
  const meta = pageMeta(page, content.length);
  const view: ResourceView<Any> = {
    name: header.toLowerCase(),
    columns: columns.map((column) => ({
      ...column,
      get: (item) => toRow(item)[column.key],
    })),
    structured: (item) => camelize(item) as Record<string, unknown>,
    pretty: {
      list(items) {
        ui.section(`${header}:`);
        for (const item of items) console.log(prettyLine(item));
        console.log();
      },
      detail() {},
    },
  };
  const total = typeof meta.total === 'number' ? meta.total : undefined;
  const number = Number(meta.number ?? 0);
  const size = Number(meta.size ?? content.length);
  const next = total !== undefined && (number + 1) * size < total ? (number + 1) * size : undefined;
  emitViewList(view, content, fmt, { page: { returned: content.length, total, next } });
}

function emitDetail(
  item: Any,
  fmt: OutputFormat,
  fields: { label: string; value: string | number | undefined }[],
  title: string,
): void {
  const view: ResourceView<Any> = {
    name: title.toLowerCase(),
    columns: [],
    structured: (value) => camelize(value) as Record<string, unknown>,
    pretty: {
      list() {},
      detail() {
        ui.section(`${title}:`);
        ui.keyValue(
          fields
            .filter(
              (field) => field.value !== undefined && field.value !== null && field.value !== '',
            )
            .map((field) => [field.label, field.value]),
        );
        console.log();
      },
    },
  };
  emitViewDetail(view, item, fmt);
}

function ackObject(verb: string, item: Any): Record<string, unknown> {
  const out: Record<string, unknown> = { action: verb };
  if (item?.id != null) out.id = item.id;
  if (item?.name != null) out.name = item.name;
  if (item?.type != null) out.type = item.type;
  if (item?.status != null) out.status = item.status;
  if (item?.profile_status != null) out.status = item.profile_status;
  if (item?.version != null) out.version = item.version;
  return out;
}

function emitAck(verb: string, item: Any, fmt: OutputFormat): void {
  if (fmt === 'json' || fmt === 'yaml') {
    emitStructured(ackObject(verb, item), fmt);
    return;
  }
  if (fmt === 'table' || fmt === 'csv') {
    const row = ackObject(verb, item);
    const cols = Object.keys(row).map((k) => ({ key: k, label: k }));
    console.log(formatOutput([row], cols, fmt));
    return;
  }
  const ver = item?.version != null ? ` v${item.version}` : '';
  ui.success(`${verb} ${item?.id ?? ''}  ${item?.name ?? ''}${ver}`);
}

function emitIdAck(verb: string, id: string): void {
  ui.success(`${verb} ${id}`);
}

// ---------------------------------------------------------------------------
// Filtering Profiles
// ---------------------------------------------------------------------------

export const dlpFilteringProfiles = {
  renderList(page: Any, fmt: OutputFormat) {
    emitList(
      page,
      fmt,
      'Data Filtering Profiles',
      (it) => ({
        id: it.id,
        name: it.name,
        type: it.type,
        direction: it.direction,
        severity: it.log_severity,
        version: it.version,
      }),
      [
        { key: 'id', label: 'ID' },
        { key: 'name', label: 'Name' },
        { key: 'type', label: 'Type' },
        { key: 'direction', label: 'Direction' },
        { key: 'severity', label: 'Severity' },
        { key: 'version', label: 'Version' },
      ],
      (it) => {
        const dir = it.direction ? chalk.dim(` dir:${it.direction}`) : '';
        const sev = it.log_severity ? chalk.dim(` sev:${it.log_severity}`) : '';
        const ver = it.version != null ? chalk.dim(` v${it.version}`) : '';
        return `  ${chalk.dim(it.id)}\n    ${it.name}  ${chalk.cyan(it.type ?? '')}${dir}${sev}${ver}`;
      },
    );
  },
  renderGet(item: Any, fmt: OutputFormat) {
    emitDetail(
      item,
      fmt,
      [
        { label: 'ID', value: item?.id },
        { label: 'Name', value: item?.name },
        { label: 'Type', value: item?.type },
        { label: 'Data Profile', value: item?.data_profile_id },
        { label: 'Direction', value: item?.direction },
        { label: 'Severity', value: item?.log_severity },
        { label: 'Scan Type', value: item?.scan_type },
        { label: 'File Based', value: item?.file_based ? 'yes' : 'no' },
        { label: 'Non-File Based', value: item?.non_file_based ? 'yes' : 'no' },
        { label: 'Version', value: item?.version },
        {
          label: 'File Types',
          value: Array.isArray(item?.file_type) ? item.file_type.length : undefined,
        },
        { label: 'Updated', value: ts(item?.audit_metadata?.updated_at) },
      ],
      'Data Filtering Profile',
    );
  },
  renderReplaced(item: Any, fmt: OutputFormat) {
    emitAck('replaced', item, fmt);
  },
};

// ---------------------------------------------------------------------------
// Patterns
// ---------------------------------------------------------------------------

export const dlpPatterns = {
  renderList(page: Any, fmt: OutputFormat) {
    emitList(
      page,
      fmt,
      'Data Patterns',
      (it) => ({
        id: it.id,
        name: it.name,
        type: it.type,
        status: it.status,
        technique: it.detection_config?.technique,
        version: it.version,
      }),
      [
        { key: 'id', label: 'ID' },
        { key: 'name', label: 'Name' },
        { key: 'type', label: 'Type' },
        { key: 'status', label: 'Status' },
        { key: 'technique', label: 'Technique' },
        { key: 'version', label: 'Version' },
      ],
      (it) => {
        const tech = it.detection_config?.technique
          ? chalk.dim(` ${it.detection_config.technique}`)
          : '';
        const ver = it.version != null ? chalk.dim(` v${it.version}`) : '';
        return `  ${chalk.dim(it.id)}\n    ${it.name}  ${chalk.cyan(it.type ?? '')}  ${statusColor(it.status)}${tech}${ver}`;
      },
    );
  },
  renderGet(item: Any, fmt: OutputFormat) {
    emitDetail(
      item,
      fmt,
      [
        { label: 'ID', value: item?.id },
        { label: 'Name', value: item?.name },
        { label: 'Description', value: item?.description },
        { label: 'Type', value: item?.type },
        { label: 'Status', value: item?.status },
        { label: 'Technique', value: item?.detection_config?.technique },
        {
          label: 'Confidence',
          value:
            (item?.detection_config?.supported_confidence_levels ?? []).join(', ') || undefined,
        },
        {
          label: 'Regexes',
          value: Array.isArray(item?.matching_rules?.regexes)
            ? item.matching_rules.regexes.length
            : undefined,
        },
        { label: 'Version', value: item?.version },
        { label: 'Updated', value: ts(item?.audit_metadata?.updated_at) },
      ],
      'Data Pattern',
    );
  },
  renderCreated(item: Any, fmt: OutputFormat) {
    emitAck('created', item, fmt);
  },
  renderReplaced(item: Any, fmt: OutputFormat) {
    emitAck('replaced', item, fmt);
  },
  renderPatched(item: Any, fmt: OutputFormat) {
    emitAck('patched', item, fmt);
  },
  renderArchived(id: string) {
    emitIdAck('archived', id);
  },
};

// ---------------------------------------------------------------------------
// Profiles (Data Profiles)
// ---------------------------------------------------------------------------

export const dlpProfiles = {
  renderList(page: Any, fmt: OutputFormat) {
    emitList(
      page,
      fmt,
      'Data Profiles',
      (it) => ({
        id: it.id,
        name: it.name,
        type: it.type,
        profile_type: it.profile_type,
        status: it.profile_status,
        version: it.version,
      }),
      [
        { key: 'id', label: 'ID' },
        { key: 'name', label: 'Name' },
        { key: 'type', label: 'Type' },
        { key: 'profile_type', label: 'Profile Type' },
        { key: 'status', label: 'Status' },
        { key: 'version', label: 'Version' },
      ],
      (it) => {
        const ptype = it.profile_type ? chalk.dim(` ${it.profile_type}`) : '';
        const ver = it.version != null ? chalk.dim(` v${it.version}`) : '';
        return `  ${chalk.dim(it.id)}\n    ${it.name}  ${chalk.cyan(it.type ?? '')}  ${statusColor(it.profile_status)}${ptype}${ver}`;
      },
    );
  },
  renderGet(item: Any, fmt: OutputFormat) {
    emitDetail(
      item,
      fmt,
      [
        { label: 'ID', value: item?.id },
        { label: 'Name', value: item?.name },
        { label: 'Description', value: item?.description },
        { label: 'Type', value: item?.type },
        { label: 'Profile Type', value: item?.profile_type },
        { label: 'Status', value: item?.profile_status },
        { label: 'Version', value: item?.version },
        { label: 'Updated', value: ts(item?.audit_metadata?.updated_at) },
      ],
      'Data Profile',
    );
  },
  renderCreated(item: Any, fmt: OutputFormat) {
    emitAck('created', item, fmt);
  },
  renderReplaced(item: Any, fmt: OutputFormat) {
    emitAck('replaced', item, fmt);
  },
  renderPatched(item: Any, fmt: OutputFormat) {
    emitAck('patched', item, fmt);
  },
};

// ---------------------------------------------------------------------------
// Dictionaries
// ---------------------------------------------------------------------------

export const dlpDictionaries = {
  renderList(page: Any, fmt: OutputFormat) {
    emitList(
      page,
      fmt,
      'Data Dictionaries',
      (it) => ({
        id: it.id,
        name: it.name,
        type: it.type,
        status: it.status,
        keywords: Array.isArray(it.keywords) ? it.keywords.length : undefined,
        version: it.version,
      }),
      [
        { key: 'id', label: 'ID' },
        { key: 'name', label: 'Name' },
        { key: 'type', label: 'Type' },
        { key: 'status', label: 'Status' },
        { key: 'keywords', label: 'Keywords' },
        { key: 'version', label: 'Version' },
      ],
      (it) => {
        const kw = Array.isArray(it.keywords) ? chalk.dim(` ${it.keywords.length} kw`) : '';
        const ver = it.version != null ? chalk.dim(` v${it.version}`) : '';
        return `  ${chalk.dim(it.id)}\n    ${it.name}  ${chalk.cyan(it.type ?? '')}  ${statusColor(it.status)}${kw}${ver}`;
      },
    );
  },
  renderGet(item: Any, fmt: OutputFormat) {
    emitDetail(
      item,
      fmt,
      [
        { label: 'ID', value: item?.id },
        { label: 'Name', value: item?.name },
        { label: 'Description', value: item?.description },
        { label: 'Type', value: item?.type },
        { label: 'Status', value: item?.status },
        {
          label: 'Keywords',
          value: Array.isArray(item?.keywords) ? item.keywords.length : undefined,
        },
        { label: 'Version', value: item?.version },
        { label: 'Updated', value: ts(item?.audit_metadata?.updated_at) },
      ],
      'Data Dictionary',
    );
  },
  renderCreated(item: Any, fmt: OutputFormat) {
    emitAck('created', item, fmt);
  },
  renderReplaced(item: Any, fmt: OutputFormat) {
    emitAck('replaced', item, fmt);
  },
  renderPatched(item: Any, fmt: OutputFormat) {
    emitAck('patched', item, fmt);
  },
  renderReplaced204Fallback(id: string) {
    ui.success(`replaced ${id} (state not echoed by region)`);
  },
  renderDeleted(id: string) {
    emitIdAck('deleted', id);
  },
};
