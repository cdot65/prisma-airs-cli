import type { AIGatewayClient, AIGatewayWindowOptions } from '@cdot65/prisma-airs-sdk';
import type { EnvironmentReport, EnvironmentReportTable, ReportSource } from './types.js';

export type GatewayReportClient = Pick<
  AIGatewayClient,
  'telemetry' | 'workspaces' | 'configs' | 'apiKeys' | 'organisations' | 'guardrails'
>;
export interface GatewayReportOptions {
  workspace: string;
  tsgId: string;
  start: Date;
  end: Date;
  title?: string;
  maxPages?: number;
  now?: () => Date;
}

function record(v: unknown): Record<string, unknown> {
  if (!v || typeof v !== 'object' || Array.isArray(v)) throw new Error('Invalid object');
  return v as Record<string, unknown>;
}
function numeric(v: unknown, integer = false): number | null {
  if (v === null) return null;
  if (
    typeof v !== 'number' ||
    !Number.isFinite(v) ||
    (integer && (!Number.isSafeInteger(v) || v < 0))
  )
    throw new Error('Invalid number');
  return v;
}
function count(v: unknown): number {
  const n = numeric(v, true);
  if (n === null) throw new Error('Missing count');
  return n;
}
const CHARTS = {
  requests: ['total'],
  errors: ['total'],
  users: ['total'],
  cost: ['total', 'avg'],
  latency: ['total', 'p50', 'p90', 'p99'],
  tokens: ['total', 'total_request_units', 'total_response_units'],
  cacheSummary: [
    'summary.cacheHits',
    'summary.avgCacheLatency',
    'summary.totalRequests',
    'summary.cacheSpeedup',
  ],
  cacheHitTrend: ['summary.totalCacheHits', 'summary.hitRate'],
  userTrends: ['summary.total', 'summary.unique', 'summary.avg'],
  errorTrends: ['summary.errorPercent'],
  errorCategoryTrends: ['summary.totalErrors', 'summary.totalUniqueErrorCodes'],
  groupedErrors: ['total'],
  rescuedRetries: ['total', 'retryTotal', 'fallbackTotal'],
  feedbackTrend: ['total'],
  feedbackWeighted: ['total'],
  feedbackScoreDistribution: ['total'],
  feedbackModels: [],
} as const;

/** Collect all 25 supplied read feeds; retain only allowlisted aggregate evidence. */
export async function collectGatewayEnvironmentReport(
  client: GatewayReportClient,
  options: GatewayReportOptions,
): Promise<EnvironmentReport> {
  const maxPages = options.maxPages ?? 40;
  if (!Number.isSafeInteger(maxPages) || maxPages < 1 || maxPages > 100)
    throw new Error('maxPages must be 1–100');
  if (
    !options.workspace.trim() ||
    !options.tsgId.trim() ||
    !Number.isFinite(options.start.getTime()) ||
    !Number.isFinite(options.end.getTime()) ||
    options.start >= options.end
  )
    throw new Error('Invalid report scope');
  const now = options.now ?? (() => new Date());
  const report: EnvironmentReport = {
    schemaVersion: 1,
    product: 'Prisma AIRS AI Gateway',
    schemaLabel: 'AI Gateway',
    windowLabel: 'Telemetry window (UTC)',
    title: options.title ?? 'AI Gateway daily report',
    collectionStartedAt: now().toISOString(),
    generatedAt: '',
    window: { start: options.start.toISOString(), end: options.end.toISOString() },
    health: 'unknown',
    sources: [],
    findings: [],
    tables: [],
    limitations: [
      'Read-only operational evidence, not a security certification or availability SLA. No inference or configuration changes are performed.',
      'All telemetry uses the displayed fixed window. Configuration and organisation/catalog reads describe current state, not daily changes.',
      'Requests, errors, feedback and log totals are independent server aggregates, not an atomic snapshot. An AIRS block is not necessarily an outage.',
      'Cost values are cents; latency values are milliseconds. Feedback scores may be negative. Null means unknown, not zero.',
      'Only aggregate numbers and HTTP status codes are retained. Keys, user identities, workspace names/IDs, metadata, URLs, prompts, raw logs and configuration bodies are excluded.',
      'Catalog entries describe available capabilities, not enabled protection. Organisation limit sentinel values are not interpreted as quota exhaustion.',
      'Incomplete sources and page budgets are explicit. Current inventory lists have no verified continuation contract; total/has_more mismatches are partial.',
    ],
  };
  const metrics: EnvironmentReportTable = {
    title: 'Telemetry metrics',
    note: 'Server field names are preserved. cost: cents; latency and avgCacheLatency: ms; errorPercent: percent. Feedback is not a request count.',
    headers: ['Source', 'Metric', 'Value'],
    rows: [],
  };
  const trends: EnvironmentReportTable = {
    title: 'Error timeline',
    note: 'Server UTC buckets; counts by HTTP status. Not a count of unique violating sessions.',
    headers: ['UTC bucket', 'HTTP status', 'Errors'],
    rows: [],
  };
  const statuses: EnvironmentReportTable = {
    title: 'Error categories',
    note: 'Server-reported errors by HTTP status; 446 denotes an AIRS block.',
    headers: ['HTTP status', 'Errors'],
    rows: [],
  };
  const inventory: EnvironmentReportTable = {
    title: 'Current configuration and capabilities',
    note: 'Aggregate counts only. Active tenant workspaces are tenant-wide; keys and configs use the selected workspace.',
    headers: ['Source', 'Metric', 'Value'],
    rows: [],
  };
  report.tables.push(metrics, statuses, trends, inventory);
  async function source(name: string, window: string, collect: (s: ReportSource) => Promise<void>) {
    const s: ReportSource = {
      name,
      method: name,
      window,
      status: 'complete',
      records: 0,
      pages: 0,
      notes: [],
    };
    report.sources.push(s);
    try {
      await collect(s);
    } catch (error) {
      s.status = s.records ? 'partial' : 'unavailable';
      const code = (error as { statusCode?: unknown } | null)?.statusCode;
      s.notes.push(
        typeof code === 'number' && Number.isInteger(code) && code >= 400 && code <= 599
          ? `HTTP ${code}: request failed; check access and service availability.`
          : 'Request, scope or response validation failed; upstream details omitted.',
      );
    }
  }
  function envelope(v: unknown, s: ReportSource): Record<string, unknown> {
    const r = record(v);
    if (r.success !== true) throw new Error('Unsuccessful response');
    const d = record(r.data);
    if (d.isQuotaExceeded !== false) {
      s.status = 'partial';
      s.notes.push('Quota exceeded or quota state unknown; aggregates may be incomplete.');
    }
    return d;
  }
  function list(v: unknown, s: ReportSource): Record<string, unknown>[] {
    const r = record(v);
    if (r.success === false || !Array.isArray(r.data)) throw new Error('Invalid list');
    const rows = r.data.map(record);
    const total = count(r.total);
    if (r.has_more === true || total !== rows.length) {
      s.status = 'partial';
      s.notes.push('List is truncated or total differs; continuation is not documented.');
    }
    s.records = rows.length;
    s.pages = 1;
    return rows;
  }
  let workspaceId: string | undefined;
  let workspaceSlug: string | undefined;
  await source('workspaces.list', 'Current active tenant workspaces (admin)', async (s) => {
    const rows = list(await client.workspaces.list({ plane: 'admin' }), s);
    inventory.rows.push([s.name, 'Active workspaces returned', rows.length]);
    const exact = rows.filter((r) => r.id === options.workspace || r.slug === options.workspace);
    const matches = exact.length ? exact : rows.filter((r) => r.name === options.workspace);
    if (
      matches.length !== 1 ||
      typeof matches[0]?.id !== 'string' ||
      typeof matches[0]?.slug !== 'string'
    )
      throw new Error('Workspace missing or ambiguous');
    workspaceId = matches[0].id;
    workspaceSlug = matches[0].slug;
  });
  const window = (): AIGatewayWindowOptions => {
    if (!workspaceSlug) throw new Error('Workspace unresolved');
    return { workspaceSlug, start: new Date(options.start), end: new Date(options.end) };
  };
  const scope = () => {
    if (!workspaceId) throw new Error('Workspace unresolved');
    return { workspaceId };
  };
  for (const [method, fields] of Object.entries(CHARTS)) {
    await source(`telemetry.${method}`, 'Displayed telemetry window', async (s) => {
      const d = envelope(await client.telemetry[method as keyof typeof CHARTS](window()), s);
      const rows: EnvironmentReportTable['rows'] = fields.map((field) => {
        const value = field.split('.').reduce<unknown>((v, key) => record(v)[key], d);
        const n = numeric(value);
        const signed = method.startsWith('feedback');
        const fractional =
          signed ||
          method === 'cost' ||
          method === 'latency' ||
          field.endsWith('avg') ||
          field.endsWith('Latency') ||
          field.endsWith('Speedup') ||
          field.endsWith('Rate') ||
          field.endsWith('Percent');
        if (n !== null && ((!signed && n < 0) || (!fractional && !Number.isSafeInteger(n))))
          throw new Error('Invalid metric');
        return [method, field, n];
      });
      const buckets = d.records ?? d.trend;
      if (buckets !== undefined && !Array.isArray(buckets)) throw new Error('Invalid buckets');
      s.records = Array.isArray(buckets) ? buckets.length : 1;
      s.pages = 1;
      if (method === 'feedbackModels') rows.push([method, 'Models with feedback', s.records]);
      if (method === 'errorCategoryTrends') {
        const entries = (d.trend as unknown[]).map((v) => {
          const r = record(v);
          return [count(r.response_status_code), count(r.count)];
        });
        statuses.rows.push(...entries);
        if (entries.some(([code, n]) => code >= 200 && code < 300 && n > 0))
          report.findings.push({
            priority: 'review',
            title: 'Success-status responses appear in error analytics',
            evidence:
              'The server classifies some HTTP 2xx responses as errors. Error analytics are not identical to non-2xx transaction counts.',
            recommendation:
              'Inspect the underlying application/provider error classifications before calculating an HTTP failure rate.',
            source: s.name,
          });
        if (entries.some(([code, n]) => n > 0 && (code === 429 || code >= 500)))
          report.findings.push({
            priority: 'attention',
            title: 'Rate limiting or server errors observed',
            evidence: 'Error categories include HTTP 429 or 5xx responses in the selected window.',
            recommendation:
              'Review provider availability, quotas and retry policies; correlate with transaction logs.',
            source: s.name,
          });
        else if (entries.some(([, n]) => n > 0))
          report.findings.push({
            priority: 'review',
            title: 'Gateway errors or policy blocks observed',
            evidence: 'Nonzero error-category counts were returned.',
            recommendation:
              'Review status categories; distinguish expected AIRS blocks from integration failures.',
            source: s.name,
          });
      }
      if (method === 'groupedErrors') {
        const entries: EnvironmentReportTable['rows'] = [];
        for (const v of d.trend as unknown[]) {
          const r = record(v);
          if (typeof r.x !== 'string' || !Number.isFinite(Date.parse(r.x)) || !Array.isArray(r.y))
            throw new Error('Invalid trend');
          for (const v of r.y) {
            const y = record(v);
            entries.push([
              new Date(r.x).toISOString(),
              count(y.response_status_code),
              count(y.count),
            ]);
          }
        }
        trends.rows.push(...entries);
      }
      metrics.rows.push(...rows);
    });
  }
  await source('telemetry.filterBoundaries', 'Displayed telemetry window', async (s) => {
    const d = envelope(await client.telemetry.filterBoundaries(window()), s);
    for (const key of ['unique_ai_models', 'unique_status_codes'] as const) {
      if (!Array.isArray(d[key])) throw new Error('Invalid filter values');
      metrics.rows.push(['filterBoundaries', `${key} count`, d[key].length]);
    }
    s.records = 1;
    s.pages = 1;
  });
  const logs: EnvironmentReportTable = {
    title: 'Transaction coverage',
    note: 'Unique collected transactions by HTTP status; only complete pagination represents the entire window. Raw rows are excluded.',
    headers: ['Metric', 'Value'],
    rows: [],
  };
  report.tables.push(logs);
  await source('telemetry.logs', 'Displayed telemetry window', async (s) => {
    const ids = new Set<string>();
    const codes = new Map<number, number>();
    let expected: number | undefined;
    try {
      for (let currentPage = 0; currentPage < maxPages; currentPage++) {
        const d = envelope(
          await client.telemetry.logs({ ...window(), pageSize: 50, currentPage }),
          s,
        );
        s.pages++;
        const total = count(d.total);
        expected ??= total;
        if (total !== expected || !Array.isArray(d.records) || d.records.length > 50)
          throw new Error('Unstable page');
        for (const v of d.records) {
          const r = record(v);
          if (
            typeof r.id !== 'string' ||
            !r.id ||
            ids.has(r.id) ||
            r.workspace_slug !== workspaceSlug
          )
            throw new Error('Invalid identity');
          const timestamp = typeof r.created_at === 'string' ? Date.parse(r.created_at) : NaN;
          if (
            !Number.isFinite(timestamp) ||
            timestamp < options.start.getTime() ||
            timestamp > options.end.getTime()
          )
            throw new Error('Outside window');
          const code = count(r.response_status_code);
          ids.add(r.id);
          codes.set(code, (codes.get(code) ?? 0) + 1);
          s.records = ids.size;
        }
        if (ids.size > expected) throw new Error('Unexpected total');
        if (ids.size === expected) return;
        if (!d.records.length) throw new Error('Premature empty page');
      }
      s.status = 'partial';
      s.notes.push(
        'Transaction page budget reached. Increase --max-pages to collect the remaining rows.',
      );
    } finally {
      logs.rows.push(['Server total', expected ?? null], ['Unique collected', ids.size]);
      for (const [code, n] of [...codes].sort(([a], [b]) => a - b))
        logs.rows.push([`HTTP ${code}`, n]);
    }
  });
  for (const method of ['configs.list', 'apiKeys.listService', 'apiKeys.listUser'] as const) {
    await source(method, 'Current selected workspace configuration', async (s) => {
      const response =
        method === 'configs.list'
          ? await client.configs.list(scope())
          : method === 'apiKeys.listService'
            ? await client.apiKeys.listService(scope())
            : await client.apiKeys.listUser(scope());
      const rows = list(response, s);
      inventory.rows.push([method, 'Rows returned', rows.length]);
    });
  }
  await source('organisations.getInfo', 'Current tenant configuration', async (s) => {
    const r = await client.organisations.getInfo(options.tsgId);
    inventory.rows.push([
      s.name,
      'Configured limit categories',
      Object.keys(r.benefits.limits).length,
    ]);
    s.records = 1;
    s.pages = 1;
  });
  await source(
    'guardrails.getCatalog',
    'Current tenant capability catalog (not enabled guardrails)',
    async (s) => {
      const r = await client.guardrails.getCatalog();
      inventory.rows.push([s.name, 'Available evaluators', r.evals.length]);
      s.records = r.evals.length;
      s.pages = 1;
    },
  );
  for (const s of report.sources.filter((s) => s.status !== 'complete'))
    report.findings.push({
      priority: 'review',
      title: `Incomplete evidence: ${s.name}`,
      evidence: s.notes.join(' '),
      recommendation:
        'Resolve source access or validation problems and rerun. Do not interpret missing metrics as zero.',
      source: s.name,
    });
  const telemetryAvailable = report.sources.some(
    (s) => s.name.startsWith('telemetry.') && s.status === 'complete',
  );
  report.health = !telemetryAvailable
    ? 'unknown'
    : report.findings.some((f) => f.priority === 'attention')
      ? 'attention'
      : report.findings.length
        ? 'review'
        : 'no-findings';
  report.generatedAt = now().toISOString();
  return report;
}
