import type {
  CustomerAppWithKeys,
  DashboardApplicationsOverviewItem,
  ScanResultEntry,
  SecurityProfile,
} from '@cdot65/prisma-airs-sdk';
import type {
  ReportFinding,
  ReportSource,
  RuntimeDailyReport,
  RuntimeReportClient,
  RuntimeReportOptions,
} from './types.js';

const PAGE_SIZE = 100;
const DAY_MS = 86_400_000;

class ReportDataError extends Error {}

/** Do not put upstream errors (which can echo requests or credentials) in deliverables. */
function safeFailure(error: unknown): string {
  if (error instanceof ReportDataError) return error.message;
  const code = (error as { statusCode?: unknown } | null)?.statusCode;
  if (typeof code === 'number' && Number.isInteger(code) && code >= 400 && code <= 599) {
    if (code === 401 || code === 403)
      return `HTTP ${code}: check Management API credentials and grants.`;
    if (code === 429) return 'HTTP 429: rate limited; retry the report later.';
    return `HTTP ${code}: source request failed; verify service availability and API compatibility.`;
  }
  return 'Source request or response validation failed; check connectivity and SDK compatibility.';
}

/** Keep metadata readable without terminal controls or unbounded server strings. */
function label(value: string | null | undefined): string {
  return (value?.replace(/[\p{Cc}\p{Cf}]/gu, ' ').trim() || 'Unknown').slice(0, 240);
}

function count(value: number | null | undefined): number | null {
  return Number.isSafeInteger(value) && (value as number) >= 0 ? (value as number) : null;
}

interface Page<T> {
  items?: T[];
  next?: number;
  note?: string;
}

async function collect<T>(
  source: ReportSource,
  maxPages: number,
  read: (offset: number) => Promise<Page<T>>,
  identity: (item: T) => string,
): Promise<T[]> {
  const records = new Map<string, T>();
  let offset = 0;
  try {
    for (let page = 0; page < maxPages; page++) {
      const result = await read(offset);
      source.pages++;
      if (!Array.isArray(result.items))
        throw new ReportDataError(
          'The API returned no record array. An empty object or absent collection does not establish zero activity.',
        );
      const before = records.size;
      for (const item of result.items) records.set(identity(item), item);
      source.records = records.size;
      if (result.note) source.notes.push(result.note);
      if (records.size - before !== result.items.length) {
        source.notes.push('Overlapping or repeated records detected; duplicates excluded.');
        source.status = 'partial';
      }
      if (result.next === undefined) {
        if (source.status !== 'partial') source.status = 'complete';
        return [...records.values()];
      }
      if (!Number.isSafeInteger(result.next) || result.next <= offset || records.size === before) {
        source.notes.push('Pagination did not advance; collection stopped safely.');
        source.status = 'partial';
        return [...records.values()];
      }
      offset = result.next;
    }
    source.notes.push(`Reached the ${maxPages}-page budget; this is not a complete inventory.`);
    source.status = 'partial';
  } catch (error) {
    source.status = source.pages > 0 && records.size > 0 ? 'partial' : 'unavailable';
    source.notes.push(safeFailure(error));
  }
  return [...records.values()];
}

function source(name: string, method: string, window: string): ReportSource {
  return { name, method, window, status: 'unavailable', records: 0, pages: 0, notes: [] };
}

function inventoryNext(next: number | undefined, size: number, offset: number): number | undefined {
  // An explicit zero ends pagination; absent metadata with a full page needs a probe.
  return next === 0 ? undefined : (next ?? (size === PAGE_SIZE ? offset + size : undefined));
}

function total(values: Array<number | null>): number | null {
  if (values.some((value) => value === null)) return null;
  const result = values.reduce<number>((sum, value) => sum + (value ?? 0), 0);
  return count(result);
}

function distribution(rows: ScanResultEntry[], key: 'action' | 'verdict') {
  const groups = new Map<string, number>();
  for (const row of rows) {
    const name = label(row[key]).toLowerCase();
    groups.set(name, (groups.get(name) ?? 0) + 1);
  }
  return [...groups].map(([name, n]) => ({ name, count: n })).sort((a, b) => b.count - a.count);
}

/**
 * Build a daily environment report through SDK reads only. No scan submissions or mutations.
 * Application sessions, log entries and current inventory remain separate units of evidence.
 */
export async function collectRuntimeDailyReport(
  client: RuntimeReportClient,
  options: RuntimeReportOptions = {},
): Promise<RuntimeDailyReport> {
  const maxPages = options.maxPages ?? 10;
  if (!Number.isSafeInteger(maxPages) || maxPages < 1 || maxPages > 100) {
    throw new Error('maxPages must be an integer from 1 to 100');
  }
  const now = options.now ?? (() => new Date());
  const started = now();
  const end = started.toISOString();
  const start = new Date(started.getTime() - DAY_MS).toISOString();
  const activitySource = source(
    'Daily application activity',
    'dashboard.applicationsOverview',
    'Rolling 1 day',
  );
  const profileSource = source(
    'Security profiles',
    'profiles.list (latest=true)',
    'Current configuration',
  );
  const appSource = source('Registered applications', 'customerApps.list', 'Current configuration');
  const logSource = source(
    'Scan log detail',
    'scanLogs.query (read-only POST)',
    'Rolling 24 hours',
  );
  let pageToken: string | undefined;
  const seenTokens = new Set<string>();
  const [applications, profileRows, registered, rawLogs] = await Promise.all([
    collect<DashboardApplicationsOverviewItem>(
      activitySource,
      maxPages,
      async (offset) => {
        const page = await client.dashboard.applicationsOverview({
          timeInterval: 1,
          timeUnit: 'day',
          limit: PAGE_SIZE,
          offset,
        });
        const size = page.items?.length ?? 0;
        const totalItems = count(page.pagination?.total_items);
        if (totalItems !== null && totalItems > offset + size && size === 0) {
          throw new ReportDataError('The API returned an empty page before its advertised total.');
        }
        return {
          items: page.items,
          next:
            totalItems !== null
              ? offset + size < totalItems
                ? offset + size
                : undefined
              : size === PAGE_SIZE
                ? offset + size
                : undefined,
        };
      },
      (item) => JSON.stringify([item.id, item.name]),
    ),
    collect<SecurityProfile>(
      profileSource,
      maxPages,
      async (offset) => {
        const page = await client.profiles.list({ latest: true, limit: PAGE_SIZE, offset });
        return {
          items: page.ai_profiles,
          next: inventoryNext(page.next_offset, page.ai_profiles.length, offset),
        };
      },
      (item) => JSON.stringify([item.profile_name, item.revision]),
    ),
    collect<CustomerAppWithKeys>(
      appSource,
      maxPages,
      async (offset) => {
        const page = await client.customerApps.list({ limit: PAGE_SIZE, offset });
        return {
          items: page.customer_apps,
          next: inventoryNext(page.next_offset, page.customer_apps?.length ?? 0, offset),
        };
      },
      (item) => item.customer_appId,
    ),
    collect<ScanResultEntry>(
      logSource,
      maxPages,
      async (offset) => {
        const pageNumber = offset + 1;
        const page = await client.scanLogs.query({
          time_interval: 24,
          time_unit: 'hours',
          pageNumber,
          pageSize: PAGE_SIZE,
          filter: 'all',
          page_token: pageToken,
        });
        const items = page.scan_result_for_dashboard?.scan_result_entries;
        if (page.page_number !== undefined && page.page_number !== pageNumber) {
          throw new ReportDataError(
            'The scan-log response page number did not match the requested page.',
          );
        }
        const totalPages = count(page.total_pages);
        let next: number | undefined;
        if (totalPages !== null) next = pageNumber < totalPages ? offset + 1 : undefined;
        else if (page.page_token || items?.length === PAGE_SIZE) next = offset + 1;
        if (next !== undefined && page.page_token) {
          if (seenTokens.has(page.page_token))
            throw new ReportDataError(
              'The scan-log continuation token repeated; collection stopped safely.',
            );
          seenTokens.add(page.page_token);
          pageToken = page.page_token;
        }
        return { items, next };
      },
      (item) => JSON.stringify([item.scan_id, item.scan_sub_req_id]),
    ),
  ]);

  const findings: ReportFinding[] = [];
  const add = (
    priority: ReportFinding['priority'],
    title: string,
    evidence: string,
    recommendation: string,
    evidenceSource: string,
  ) => {
    findings.push({ priority, title, evidence, recommendation, source: evidenceSource });
  };
  for (const s of [activitySource, profileSource, appSource, logSource]) {
    if (s.status !== 'complete')
      add(
        'review',
        `${s.name}: ${s.status}`,
        s.notes.join(' '),
        'Restore access or retry collection; do not treat missing data as zero. Increase --max-pages if the page budget was reached.',
        s.name,
      );
  }
  const appIds = new Set(registered.map((app) => app.customer_appId));
  const reportApps = applications
    .map((app) => ({
      name: label(app.name),
      sessions: count(app.sessions_total),
      violatingSessions: count(app.sessions_violated),
      registered:
        app.id && appIds.has(app.id)
          ? true
          : appSource.status === 'complete' && app.id
            ? false
            : null,
    }))
    .sort(
      (a, b) =>
        (b.violatingSessions ?? -1) - (a.violatingSessions ?? -1) || a.name.localeCompare(b.name),
    );
  const sessions =
    activitySource.status === 'unavailable' ? null : total(reportApps.map((app) => app.sessions));
  const violatingSessions =
    activitySource.status === 'unavailable'
      ? null
      : total(reportApps.map((app) => app.violatingSessions));
  const inconsistent = reportApps.some(
    (app) =>
      app.sessions !== null &&
      app.violatingSessions !== null &&
      app.violatingSessions > app.sessions,
  );
  if (
    inconsistent ||
    reportApps.some((app) => app.sessions === null || app.violatingSessions === null)
  ) {
    add(
      'review',
      'Activity counters need verification',
      'Some application counters are absent, invalid, or report more violating sessions than total sessions.',
      'Compare these buckets with the SCM dashboard. A violation rate is withheld when counters are inconsistent.',
      activitySource.name,
    );
  }
  if ((violatingSessions ?? 0) > 0)
    add(
      'attention',
      'Violating sessions observed',
      `${violatingSessions} violating sessions across ${reportApps.filter((app) => (app.violatingSessions ?? 0) > 0).length} application buckets in the collected daily data.`,
      'Review the highest-volume applications in SCM, confirm intended enforcement, and investigate unexpected activity. Violations are not proof of a successful attack.',
      activitySource.name,
    );
  if (sessions === 0 && activitySource.status === 'complete')
    add(
      'review',
      'No sessions reported in the daily window',
      'The complete application overview reports zero sessions.',
      'If traffic was expected, check the integration, application metadata, and telemetry ingestion. No activity is not proof of health.',
      activitySource.name,
    );

  // Latest-only is requested upstream; additionally collapse revisions by name defensively.
  const latestProfiles = new Map<string, SecurityProfile>();
  for (const profile of profileRows) {
    const old = latestProfiles.get(profile.profile_name);
    if (!old || (profile.revision ?? -1) > (old.revision ?? -1))
      latestProfiles.set(profile.profile_name, profile);
  }
  const profiles = [...latestProfiles.values()]
    .map((profile) => {
      const configs = (profile.policy?.['ai-security-profiles'] ?? []).flatMap((p) =>
        p['model-configuration'] ? [p['model-configuration']] : [],
      );
      const timeoutActions = [
        ...new Set(configs.map((c) => label(c.latency?.['inline-timeout-action']))),
      ];
      const name = label(profile.profile_name);
      if (profile.active === false)
        add(
          'review',
          `Inactive profile: ${name}`,
          'The latest returned revision is explicitly inactive.',
          'Confirm this is intentional before assigning the profile to an integration.',
          profileSource.name,
        );
      if (
        profile.active === true &&
        timeoutActions.some((action) => action.toLowerCase() === 'allow')
      )
        add(
          'review',
          `Timeout allows traffic: ${name}`,
          'An active profile explicitly sets inline-timeout-action to allow.',
          'Review the availability-versus-enforcement tradeoff with the application owner; this is not evidence of a timeout or bypass.',
          profileSource.name,
        );
      const masking = configs.map((c) => c['mask-data-in-storage']);
      const storageMasking = masking.includes(false)
        ? 'Off in at least one configuration'
        : masking.length > 0 && masking.every((v) => v === true)
          ? 'On in all returned configurations'
          : 'Unknown';
      if (profile.active === true && masking.includes(false))
        add(
          'review',
          `Storage masking disabled: ${name}`,
          'An active profile explicitly disables mask-data-in-storage in a model configuration.',
          'Confirm retention and privacy requirements. This flag alone does not establish that sensitive data was stored.',
          profileSource.name,
        );
      return {
        name,
        revision: count(profile.revision),
        active: profile.active ?? null,
        modified:
          profile.last_modified_ts && Number.isFinite(Date.parse(profile.last_modified_ts))
            ? new Date(profile.last_modified_ts).toISOString()
            : null,
        timeoutActions,
        storageMasking,
      };
    })
    .sort((a, b) => a.name.localeCompare(b.name));
  if (profiles.length === 0 && profileSource.status === 'complete')
    add(
      'attention',
      'No security profiles returned',
      'The current profile inventory is empty.',
      'Verify tenant scope and configure a security profile before routing application scans.',
      profileSource.name,
    );

  let missingTimestamps = 0;
  let outsideWindow = 0;
  const logs = rawLogs.filter((row) => {
    const timestamp = Date.parse(row.received_ts ?? '');
    if (!Number.isFinite(timestamp)) {
      missingTimestamps++;
      return false;
    }
    if (timestamp < Date.parse(start) || timestamp >= Date.parse(end)) {
      outsideWindow++;
      return false;
    }
    return true;
  });
  if (missingTimestamps || outsideWindow)
    add(
      'review',
      'Some log records excluded from daily analysis',
      `${missingTimestamps} records lack a valid received timestamp; ${outsideWindow} fall outside the anchored UTC window.`,
      'Verify ingestion timestamps and collection-window drift before using log counts as daily totals.',
      logSource.name,
    );

  const failedLogs = logs.filter((row) =>
    ['failed', 'error', 'timeout'].includes(row.status?.trim().toLowerCase() ?? ''),
  ).length;
  if (failedLogs)
    add(
      'attention',
      'Scan processing failures observed',
      `${failedLogs} timestamp-eligible collected log entries explicitly report failed, error, or timeout status.`,
      'Review these failures in SCM, check integration error handling, and verify whether the affected application fails open or closed. This is not a tenant-wide failure rate.',
      logSource.name,
    );
  const priorities = { attention: 0, review: 1, info: 2 };
  findings.sort((left, right) => priorities[left.priority] - priorities[right.priority]);

  return {
    schemaVersion: 1,
    product: 'Prisma AIRS AI Runtime Security',
    title: label(options.title ?? 'Daily environment report'),
    generatedAt: now().toISOString(),
    collectionStartedAt: end,
    window: {
      start,
      end,
      description:
        'Rolling last 24 hours, UTC (approximate for server-relative application queries)',
    },
    health: [activitySource, profileSource, appSource, logSource].every(
      (s) => s.status === 'unavailable',
    )
      ? 'unknown'
      : findings.some((f) => f.priority === 'attention')
        ? 'attention'
        : findings.some((f) => f.priority === 'review')
          ? 'review'
          : 'no-findings',
    sources: [activitySource, profileSource, appSource, logSource],
    findings,
    activity: {
      sessions,
      violatingSessions,
      violationRate:
        !inconsistent && sessions !== null && sessions > 0 && violatingSessions !== null
          ? (violatingSessions / sessions) * 100
          : null,
      applications: reportApps,
    },
    profiles,
    registeredApps: registered
      .map((app) => ({
        name: label(app.app_name),
        environment: label(app.environment),
        cloud: label(app.cloud_provider),
        model: label(app.model_name),
        keyAssociations: app.api_keys_dp_info ? app.api_keys_dp_info.length : null,
      }))
      .sort((a, b) => a.name.localeCompare(b.name)),
    logs: {
      entries: logs.length,
      actions: distribution(logs, 'action'),
      verdicts: distribution(logs, 'verdict'),
      tokens:
        logSource.status === 'unavailable' ? null : total(logs.map((row) => count(row.tokens))),
      missingTimestamps,
      outsideWindow,
    },
    limitations: [
      'This is a read-only operational review, not an uptime SLA, compliance attestation, or proof that attacks succeeded or were blocked. No health score is invented.',
      'Application activity uses the API’s rolling one-day window; each paginated request evaluates its own server-relative window. Collection is not a transactionally consistent historical snapshot. Ingestion may lag.',
      'Sessions, scan-log entries, text records, and API calls are different units. They are not added together. Log counters describe only collected, timestamp-eligible entries; absent data is unknown.',
      'Application buckets are keyed by registered application ID plus the literal scan metadata.app_name, which can differ from registered names. Do not sum session counts as unique users or unique tenant-wide sessions.',
      'Current configuration is not a configuration-change audit. Missing policy settings are unknown, not disabled. Inactive profiles can be intentional; current profile state may differ from the revision used by a historical scan.',
      'Per-app token summaries and detector/severity breakdowns require 7/30/60-day windows, so they are not presented as daily metrics. No previous-day comparison or trend is inferred.',
      'Prompts, responses, user identities/IPs, API keys, auth codes, tenant IDs, and raw errors are omitted. Application/profile names and configuration metadata remain confidential; review before sharing.',
    ],
  };
}
