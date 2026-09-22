import type { QuotaSummary, RedTeamClient } from '@cdot65/prisma-airs-sdk';
import type { ReportFinding, ReportSource, RuntimeReportOptions } from './types.js';

/** Read-only SDK surface; older SDK installations explicitly report quota as unavailable. */
export type RedTeamReportClient = Pick<
  RedTeamClient,
  'getDashboardOverview' | 'getScanStatistics'
> & {
  targets: Pick<RedTeamClient['targets'], 'list'>;
  scans: Pick<RedTeamClient['scans'], 'list'>;
  adapters: Pick<RedTeamClient['adapters'], 'list'>;
  networkBroker: Pick<RedTeamClient['networkBroker'], 'getChannelStats'>;
  getQuotaSummary?: () => Promise<QuotaSummary>;
};

export interface RedTeamReportTable {
  title: string;
  note: string;
  headers: string[];
  rows: Array<Array<string | number | null>>;
}

/** Aggregate-only projection: no target credentials, identities, scripts, prompts or raw responses. */
export interface RedTeamEnvironmentReport {
  schemaVersion: 1;
  product: 'Prisma AIRS AI Red Teaming';
  title: string;
  collectionStartedAt: string;
  generatedAt: string;
  window: { start: string; end: string };
  health: 'attention' | 'review' | 'no-findings' | 'unknown';
  sources: ReportSource[];
  findings: ReportFinding[];
  tables: RedTeamReportTable[];
  limitations: string[];
}

class DataError extends Error {}
const label = (value: unknown) =>
  typeof value === 'string' ? value.replace(/[\p{Cc}\p{Cf}]/gu, ' ').slice(0, 120) : 'Unknown';
function count(value: unknown): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0)
    throw new DataError('A required counter is missing or invalid.');
  return value;
}
function failure(error: unknown): string {
  if (error instanceof DataError) return error.message;
  const code = (error as { statusCode?: unknown } | null)?.statusCode;
  return typeof code === 'number' && Number.isInteger(code) && code >= 400 && code <= 599
    ? `HTTP ${code}: source unavailable; check credentials, service availability and SDK compatibility.`
    : 'Source request or validation failed. No upstream error body is included.';
}

/** Collect seven read-only feeds, with explicit source budgets and independent metric windows. */
export async function collectRedTeamEnvironmentReport(
  client: RedTeamReportClient,
  options: RuntimeReportOptions = {},
): Promise<RedTeamEnvironmentReport> {
  const maxPages = options.maxPages ?? 40;
  if (!Number.isSafeInteger(maxPages) || maxPages < 1 || maxPages > 100)
    throw new Error('maxPages must be an integer from 1 to 100');
  const now = options.now ?? (() => new Date());
  const end = now();
  const report: RedTeamEnvironmentReport = {
    schemaVersion: 1,
    product: 'Prisma AIRS AI Red Teaming',
    title: options.title ?? 'Red Team environment report',
    collectionStartedAt: end.toISOString(),
    generatedAt: '',
    window: { start: new Date(end.getTime() - 86_400_000).toISOString(), end: end.toISOString() },
    health: 'unknown',
    sources: [],
    findings: [],
    tables: [],
    limitations: [
      'This is a read-only snapshot, not a security certification, availability SLA or numerical health score.',
      'Only scan creation timestamps are filtered locally to the preceding 24 hours. Current scan status is not a history of status changes.',
      'Dashboard statistics use server defaults with an unverified measurement window. They are not daily totals and need not match the paginated scan inventory.',
      'Targets and adapters are current configuration inventories, not configuration change logs. Broker channels not online may be intentionally idle; no outage is inferred.',
      'Risk categories are server-reported. Quota consumption is not reset or reinterpreted as daily usage; unlimited zero allocation is not exhaustion.',
      'Partial sources describe collected records only. Missing data is unknown, never zero. Independently read pages are not an atomic snapshot.',
      'Only aggregate counters and category labels are included. Target names, IDs, connection settings, scripts, variables, prompts and scan content are excluded.',
    ],
  };
  function finding(
    priority: ReportFinding['priority'],
    title: string,
    evidence: string,
    recommendation: string,
    source: string,
  ) {
    report.findings.push({ priority, title, evidence, recommendation, source });
  }
  async function source(
    name: string,
    method: string,
    window: string,
    read: (s: ReportSource) => Promise<void>,
  ) {
    const s: ReportSource = {
      name,
      method,
      window,
      status: 'unavailable',
      records: 0,
      pages: 0,
      notes: [],
    };
    report.sources.push(s);
    try {
      await read(s);
      if (s.status !== 'partial') s.status = 'complete';
    } catch (error) {
      s.status = s.records > 0 ? 'partial' : 'unavailable';
      s.notes.push(failure(error));
    }
    if (s.status !== 'complete')
      finding(
        'review',
        `${name} incomplete`,
        s.notes.join(' '),
        'Resolve the source error or increase the page budget, then regenerate.',
        name,
      );
  }
  async function pages<T extends { uuid: string }>(
    s: ReportSource,
    read: (
      skip: number,
    ) => Promise<{ data?: T[] | null; pagination: { total_items?: number | null } }>,
  ): Promise<T[]> {
    const rows: T[] = [];
    const seen = new Set<string>();
    let total: number | undefined;
    try {
      for (let page = 0; page < maxPages; page++) {
        const r = await read(rows.length);
        s.pages++;
        if (!Array.isArray(r.data)) throw new DataError('Missing inventory array.');
        const reported = count(r.pagination.total_items);
        total ??= reported;
        if (total !== reported) throw new DataError('Inventory changed during pagination.');
        if (r.data.length > 15) throw new DataError('Page exceeds requested size.');
        for (const row of r.data) {
          if (!row.uuid || seen.has(row.uuid))
            throw new DataError('Missing or repeated identity; pagination stopped.');
          seen.add(row.uuid);
          rows.push(row);
          s.records = rows.length;
        }
        if (rows.length > total) throw new DataError('Inventory exceeds reported total.');
        if (rows.length === total) return rows;
        if (!r.data.length) throw new DataError('Premature empty page.');
      }
      throw new DataError(`Reached the ${maxPages}-page budget; inventory is incomplete.`);
    } catch (error) {
      if (!rows.length) throw error;
      s.status = 'partial';
      s.notes.push(failure(error));
      return rows;
    }
  }
  const table = (
    title: string,
    note: string,
    headers: string[],
    rows: RedTeamReportTable['rows'],
  ) => report.tables.push({ title, note, headers, rows });
  function groups(values: unknown[]): RedTeamReportTable['rows'] {
    const counts = new Map<string, number>();
    for (const v of values) {
      const key = label(v);
      counts.set(key, (counts.get(key) ?? 0) + 1);
    }
    return [...counts].sort(([a], [b]) => a.localeCompare(b));
  }
  await source('Overview', 'getDashboardOverview()', 'Current dashboard snapshot', async (s) => {
    const r = await client.getDashboardOverview();
    const total = count(r.total_targets);
    table('Dashboard overview', s.window, ['Metric', 'Count'], [['Targets', total]]);
    s.records = 1;
    s.pages = 1;
  });
  await source(
    'Statistics',
    'getScanStatistics()',
    'Server-default window (unverified)',
    async (s) => {
      const r = await client.getScanStatistics();
      const rows: RedTeamReportTable['rows'] = [
        ['Scans', count(r.total_scans)],
        ['Targets scanned', count(r.targets_scanned)],
      ];
      for (const risk of r.risk_profile ?? []) {
        const n = count(risk.total);
        rows.push([`Risk: ${label(risk.risk_rating)}`, n]);
        if (/^(high|critical)$/i.test(risk.risk_rating) && n > 0)
          finding(
            'attention',
            'Elevated risk reported',
            `${n} in the ${label(risk.risk_rating)} risk category.`,
            'Review affected targets in Red Team; the statistics window is server-defined.',
            s.name,
          );
      }
      for (const status of r.scan_status ?? [])
        rows.push([`Status: ${label(status.name)}`, count(status.count)]);
      table('Dashboard scan statistics', s.window, ['Metric', 'Count'], rows);
      s.records = 1;
      s.pages = 1;
    },
  );
  await source('Targets', 'targets.list()', 'Current paginated inventory', async (s) => {
    const rows = await pages(s, (skip) => client.targets.list({ skip, limit: 15 }));
    table(
      'Target inventory',
      'Collected targets by type; consult source completeness.',
      ['Type', 'Count'],
      groups(rows.map((r) => r.target_type)),
    );
  });
  await source('Quota', 'getQuotaSummary() — GET', 'Current quota snapshot', async (s) => {
    if (!client.getQuotaSummary)
      throw new DataError(
        'Installed SDK lacks getQuotaSummary(); use the locally updated SDK. POST is not used as a fallback.',
      );
    const r = await client.getQuotaSummary();
    const rows: RedTeamReportTable['rows'] = [];
    for (const kind of ['static', 'dynamic', 'custom'] as const) {
      const q = r[kind];
      const allocated = count(q.allocated);
      const consumed = count(q.consumed);
      if (typeof q.unlimited !== 'boolean') throw new DataError('Missing unlimited quota flag.');
      rows.push([
        kind,
        allocated,
        consumed,
        q.unlimited ? 'Unlimited' : String(Math.max(0, allocated - consumed)),
      ]);
      if (!q.unlimited && consumed >= allocated)
        finding(
          'attention',
          `${kind} quota has no remaining allocation`,
          `${consumed} consumed; ${allocated} allocated.`,
          'Review quota allocation before scheduling more scans.',
          s.name,
        );
    }
    table(
      'Quota',
      'Remaining is shown only for finite quotas; these are not daily usage counters.',
      ['Type', 'Allocated', 'Consumed', 'Remaining'],
      rows,
    );
    s.records = 3;
    s.pages = 1;
  });
  await source(
    'Scans',
    'scans.list()',
    'Paginated inventory; local 24-hour creation filter',
    async (s) => {
      const rows = await pages(s, (skip) => client.scans.list({ skip, limit: 15 }));
      let unknown = 0;
      const recent = rows.filter((r) => {
        const t =
          typeof r.created_at === 'string' && /(?:Z|[+-]\d{2}:\d{2})$/i.test(r.created_at)
            ? Date.parse(r.created_at)
            : Number.NaN;
        if (!Number.isFinite(t) || t > end.getTime()) {
          unknown++;
          return false;
        }
        return t >= end.getTime() - 86_400_000;
      });
      table(
        'Scan inventory and daily creation activity',
        'Daily count covers collected records created in the stated UTC window, not scans completed or attacks executed.',
        ['Metric', 'Count'],
        [
          ['Collected scans', rows.length],
          ['Created in preceding 24 hours', recent.length],
          ['Unusable or future creation timestamps', unknown],
        ],
      );
      table(
        'Current status of collected scans',
        'All collected scans, independent of creation window.',
        ['Status', 'Count'],
        groups(rows.map((r) => r.status)),
      );
      const failed = rows.filter((r) => /^(failed|error)$/i.test(r.status ?? '')).length;
      if (failed)
        finding(
          'attention',
          'Scans require failure review',
          `${failed} collected scans currently report failed/error status.`,
          'Inspect individual scan reports and error logs before rerunning; no reruns are performed here.',
          s.name,
        );
      if (unknown)
        finding(
          'review',
          'Daily activity has timestamp gaps',
          `${unknown} records could not be assigned to the daily window.`,
          'Review source timestamps; do not interpret the daily count as complete.',
          s.name,
        );
    },
  );
  await source(
    'Network broker',
    'networkBroker.getChannelStats()',
    'Current channel snapshot',
    async (s) => {
      const r = await client.networkBroker.getChannelStats();
      const total = count(r.total_channels);
      const online = count(r.online_channels);
      if (online > total) throw new DataError('Online channel count exceeds total.');
      table(
        'Network broker',
        'Not-online channels may be intentionally idle; this does not establish an outage.',
        ['Metric', 'Count'],
        [
          ['Configured channels', total],
          ['Online channels', online],
        ],
      );
      if (total > online)
        finding(
          'review',
          'Confirm expected broker connectivity',
          `${online} of ${total} configured channels are online.`,
          'Compare with intended channel schedules before investigating connectivity.',
          s.name,
        );
      s.records = 1;
      s.pages = 1;
    },
  );
  await source(
    'Adapters',
    'adapters.list({ include_target_count: true })',
    'Current paginated configuration',
    async (s) => {
      const rows = await pages(s, (skip) =>
        client.adapters.list({ skip, limit: 15, include_target_count: true }),
      );
      table(
        'Adapter inventory',
        'Configuration status only; scripts and variables are excluded.',
        ['Status', 'Count'],
        groups(rows.map((r) => r.status)),
      );
    },
  );
  report.health = report.sources.every((s) => s.status === 'unavailable')
    ? 'unknown'
    : report.findings.some((f) => f.priority === 'attention')
      ? 'attention'
      : report.findings.length
        ? 'review'
        : 'no-findings';
  report.generatedAt = now().toISOString();
  return report;
}
