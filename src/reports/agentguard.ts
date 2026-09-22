import type { AgentGuardClient } from '@cdot65/prisma-airs-sdk';
import type { EnvironmentReport, ReportSource, RuntimeReportOptions } from './types.js';

export type AgentGuardReportClient = Pick<
  AgentGuardClient,
  'listScans' | 'getScanStats' | 'listRules'
>;

/** Aggregate-only read-only report. Never retrieves finding text, paths, code or device metadata. */
export async function collectAgentGuardReport(
  client: AgentGuardReportClient,
  options: RuntimeReportOptions = {},
): Promise<EnvironmentReport> {
  const maxPages = options.maxPages ?? 40;
  if (!Number.isSafeInteger(maxPages) || maxPages < 1 || maxPages > 100)
    throw new Error('maxPages must be between 1 and 100');
  const now = options.now ?? (() => new Date());
  const end = now();
  const window = {
    start: new Date(end.getTime() - 30 * 86400000).toISOString(),
    end: end.toISOString(),
  };
  const report: EnvironmentReport = {
    schemaVersion: 1,
    product: 'Prisma AIRS AI Supply Chain — AgentGuard',
    title: options.title ?? 'AgentGuard environment report',
    collectionStartedAt: end.toISOString(),
    generatedAt: '',
    window,
    windowLabel: '30-day scan inventory; rolling 30_DAYS statistics; current rule catalog',
    schemaLabel: 'Experimental browser APIs',
    health: 'unknown',
    sources: [],
    findings: [],
    tables: [],
    limitations: [
      'Read-only evidence, not a security certification or numerical health score. Browser APIs are undocumented and may change.',
      'Scans use explicit start/end timestamps; statistics use the server rolling 30_DAYS window. These independently read sources are not an atomic snapshot and need not reconcile.',
      'Scan rows are not unique skills. Parent/batch rows are excluded from outcome counts to avoid counting them alongside children; per-scan vulnerability counts are never summed.',
      'Rule default states describe the catalog, not effective tenant policies. No scan creation, policy changes or skill uploads are performed.',
      'Missing or incomplete evidence is unknown, not zero. Scan names, identifiers, URLs, device metadata, source code and finding text are excluded.',
    ],
  };
  const label = (v: string | null, allowed: string[]) =>
    v === null ? 'UNKNOWN' : allowed.includes(v) ? v : 'OTHER';
  async function source(
    name: string,
    method: string,
    range: string,
    read: (s: ReportSource) => Promise<void>,
  ) {
    const s: ReportSource = {
      name,
      method,
      window: range,
      status: 'unavailable',
      pages: 0,
      records: 0,
      notes: [],
    };
    report.sources.push(s);
    try {
      await read(s);
      if (s.status !== 'partial') s.status = 'complete';
    } catch {
      s.status = s.records ? 'partial' : 'unavailable';
      s.notes.push('Request or validation failed; no upstream error body included.');
    }
    if (s.status !== 'complete')
      report.findings.push({
        priority: 'review',
        title: `${name} incomplete`,
        evidence: s.notes.join(' '),
        recommendation: 'Check service access or increase the page budget, then regenerate.',
        source: name,
      });
  }
  async function pages<T extends { uuid: string }>(
    s: ReportSource,
    read: (skip: number) => Promise<{ rows: T[]; total: number }>,
    pageCount = false,
  ) {
    const rows: T[] = [];
    const seen = new Set<string>();
    let total: number | undefined;
    try {
      for (let page = 0; page < maxPages; page++) {
        const r = await read(rows.length);
        s.pages++;
        if (
          !Number.isSafeInteger(r.total) ||
          r.total < 0 ||
          !Array.isArray(r.rows) ||
          r.rows.length > 10
        )
          throw new Error();
        total ??= r.total;
        if (!pageCount && (r.total !== total || rows.length + r.rows.length > total))
          throw new Error();
        for (const row of r.rows) {
          if (!row.uuid || seen.has(row.uuid)) throw new Error();
          seen.add(row.uuid);
          rows.push(row);
          s.records = rows.length;
        }
        if (pageCount ? r.rows.length < 10 : rows.length === total) return rows;
        if (!r.rows.length) throw new Error();
      }
      s.notes.push(`Reached the ${maxPages}-page budget.`);
      throw new Error();
    } catch (error) {
      if (!rows.length) throw error;
      s.status = 'partial';
      s.notes.push(
        'Inventory incomplete: pagination changed, repeated, stopped early, failed or reached its budget.',
      );
      return rows;
    }
  }
  await source('Scans', 'listScans()', `${window.start} to ${window.end}`, async (s) => {
    const rows = await pages(s, async (skip) => {
      const p = await client.listScans({
        skip,
        limit: 10,
        start_time: window.start,
        end_time: window.end,
        isBackgroundRefresh: false,
      });
      return { rows: p.scans, total: p.pagination.total_items };
    });
    const inside = rows.filter((r) => {
      const t = Date.parse(r.created_at);
      return Number.isFinite(t) && t >= Date.parse(window.start) && t <= end.getTime();
    });
    if (inside.length !== rows.length) {
      s.status = 'partial';
      s.notes.push('Rows with missing or out-of-window creation timestamps were excluded.');
    }
    const leaf = inside.filter((r) => !r.is_batch && r.child_scan_uuids.length === 0);
    const groups = new Map<string, number>();
    for (const r of leaf) {
      const key = `${label(r.artifact_type, ['SKILL', 'AGENT'])} / ${label(r.eval_outcome, ['BLOCKED', 'ALLOWED', 'PENDING', 'ERROR'])}`;
      groups.set(key, (groups.get(key) ?? 0) + 1);
    }
    report.tables.push({
      title: 'Scan outcomes',
      note: `Collected ${s.records} rows; excluded ${inside.length - leaf.length} parent/batch rows. ${s.status === 'partial' ? 'Partial inventory only.' : 'Complete inventory for requested window.'}`,
      headers: ['Artifact / outcome', 'Scan rows'],
      rows: [...groups].sort(([a], [b]) => a.localeCompare(b)),
    });
    const blocked = leaf.filter((r) => r.eval_outcome === 'BLOCKED').length;
    if (blocked)
      report.findings.push({
        priority: 'attention',
        title: 'Blocked scans require review',
        evidence: `${blocked} collected non-parent scan rows have BLOCKED evaluations.`,
        recommendation: 'Inspect the affected scans and findings before approving the artifacts.',
        source: 'Scans',
      });
    const unsettled = leaf.filter(
      (r) => r.status !== 'COMPLETED' || !['ALLOWED', 'BLOCKED'].includes(r.eval_outcome ?? ''),
    ).length;
    if (unsettled)
      report.findings.push({
        priority: 'review',
        title: 'Scans without a completed decision',
        evidence: `${unsettled} collected non-parent rows are pending, failed or have an unfamiliar status.`,
        recommendation:
          'Review scan status; absence of a completed decision is not a passing evaluation.',
        source: 'Scans',
      });
  });
  await source('Statistics', 'getScanStats()', 'Server rolling 30_DAYS', async (s) => {
    const r = await client.getScanStats({ time_period: '30_DAYS' });
    report.tables.push({
      title: 'Skill statistics',
      note: 'Server aggregates; percentage changes may be unavailable.',
      headers: ['Metric', 'Count', 'Change (%)'],
      rows: [
        [
          'Unique skills scanned',
          r.unique_skills_scanned.count,
          r.unique_skills_scanned.percent_change,
        ],
        [
          'Vulnerabilities found',
          r.total_vulnerabilities_found.count,
          r.total_vulnerabilities_found.percent_change,
        ],
      ],
    });
    if (r.total_vulnerabilities_found.count > 0)
      report.findings.push({
        priority: 'attention',
        title: 'Skill vulnerabilities reported',
        evidence: `${r.total_vulnerabilities_found.count} findings reported in the server statistics window.`,
        recommendation:
          'Review vulnerability details and remediate before use; findings are not a count of distinct vulnerable skills.',
        source: 'Statistics',
      });
    s.records = 1;
    s.pages = 1;
  });
  await source('Rules', 'listRules()', 'Current catalog', async (s) => {
    const rows = await pages(
      s,
      async (skip) => {
        const p = await client.listRules({ skip, limit: 10 });
        return { rows: p.rules, total: p.pagination.total_items };
      },
      true,
    );
    const groups = new Map<string, number>();
    for (const r of rows) {
      const state = label(r.default_state, ['BLOCKING', 'ALLOWING']);
      groups.set(state, (groups.get(state) ?? 0) + 1);
    }
    report.tables.push({
      title: 'Rule catalog defaults',
      note: 'These are not effective tenant policy settings.',
      headers: ['Default state', 'Rules'],
      rows: [...groups].sort(([a], [b]) => a.localeCompare(b)),
    });
  });
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
