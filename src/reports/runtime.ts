import type {
  CustomerAppWithKeys,
  DashboardApplicationsOverviewItem,
  DashboardSessionOverviewItem,
  DashboardSessionsChartBucket,
  DashboardTopApplicationViolations,
  DashboardViolationsTrendBucket,
  SecurityProfile,
  ViolationSeverityCounts,
} from '@cdot65/prisma-airs-sdk';
import type {
  ReportFinding,
  ReportSeverity,
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

function distribution(rows: DashboardSessionOverviewItem[]) {
  const groups = new Map<string, number>();
  for (const row of rows) {
    const name = label(row.violation_status).toLowerCase();
    groups.set(name, (groups.get(name) ?? 0) + 1);
  }
  return [...groups].map(([name, n]) => ({ name, count: n })).sort((a, b) => b.count - a.count);
}

function severity(value: ViolationSeverityCounts): ReportSeverity {
  return {
    critical: count(value.critical),
    high: count(value.high),
    medium: count(value.medium),
    low: count(value.low),
    total: count(value.total),
  };
}

/**
 * Build a daily environment report through SDK reads only. No scan submissions or mutations.
 * Application buckets, session inventory and detector violations remain separate units of evidence.
 */
export async function collectRuntimeDailyReport(
  client: RuntimeReportClient,
  options: RuntimeReportOptions = {},
): Promise<RuntimeDailyReport> {
  const maxPages = options.maxPages ?? 40;
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
  const sessionSource = source(
    'Daily session inventory',
    'dashboard.sessionsOverview',
    'Rolling 1 day',
  );
  const chartSource = source('Daily session chart', 'dashboard.sessionsChart', 'Rolling 1 day');
  const rankingSource = source(
    'Top application violations',
    'dashboard.topApplicationsViolations',
    'Rolling 1 day; server-ranked subset',
  );
  const trendSource = source(
    'Daily violation trend',
    'dashboard.applicationsViolationsTrend',
    'Rolling 1 day',
  );
  const sources = [
    activitySource,
    profileSource,
    appSource,
    sessionSource,
    chartSource,
    rankingSource,
    trendSource,
  ];
  const daily = { timeInterval: 1, timeUnit: 'day' } as const;
  let expectedSessionTotal: number | undefined;
  let expectedAppTotal: number | undefined;
  const [applications, profileRows, registered, rawSessions, chart, rankings, trend] =
    await Promise.all([
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
          if (totalItems !== null) {
            if (expectedAppTotal !== undefined && totalItems !== expectedAppTotal)
              throw new ReportDataError(
                'Application total changed during pagination; retry for a stable inventory.',
              );
            expectedAppTotal = totalItems;
            if (offset + size > totalItems)
              throw new ReportDataError('Application records exceed the advertised total.');
          }
          if (totalItems !== null && totalItems > offset + size && size === 0) {
            throw new ReportDataError(
              'The API returned an empty page before its advertised total.',
            );
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
      collect<DashboardSessionOverviewItem>(
        sessionSource,
        maxPages,
        async (offset) => {
          const page = await client.dashboard.sessionsOverview({ ...daily, limit: 25, offset });
          const size = page.items?.length ?? 0;
          const totalItems = count(page.pagination?.total_items);
          if (
            totalItems === null ||
            page.pagination?.skip !== offset ||
            page.pagination?.limit !== 25
          )
            throw new ReportDataError('Missing or mismatched session pagination metadata.');
          if (expectedSessionTotal !== undefined && expectedSessionTotal !== totalItems)
            throw new ReportDataError(
              'Session total changed during pagination; retry for a stable inventory.',
            );
          expectedSessionTotal = totalItems;
          if (offset + size > totalItems || size > 25 || (size === 0 && offset < totalItems))
            throw new ReportDataError('Session page does not agree with its advertised total.');
          return {
            items: page.items,
            next: offset + size < totalItems ? offset + size : undefined,
          };
        },
        (item) => JSON.stringify([item.application_id, item.application_name, item.session_id]),
      ),
      collect<DashboardSessionsChartBucket>(
        chartSource,
        1,
        async () => ({ items: (await client.dashboard.sessionsChart(daily)).buckets }),
        (item) => String(item.bucket_number),
      ),
      collect<DashboardTopApplicationViolations>(
        rankingSource,
        1,
        async () => ({
          items: (await client.dashboard.topApplicationsViolations(daily)).applications,
        }),
        (item) => JSON.stringify([item.id, item.name]),
      ),
      collect<DashboardViolationsTrendBucket>(
        trendSource,
        1,
        async () => ({
          items: (await client.dashboard.applicationsViolationsTrend(daily)).violations,
        }),
        (item) => String(item.bucket_number),
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
  for (const s of sources) {
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
  const sessionRows = rawSessions.filter((row) => {
    const timestamp = Date.parse(row.last_session_activity ?? '');
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
      'Some session records excluded from daily analysis',
      `${missingTimestamps} records lack a valid last-activity timestamp; ${outsideWindow} fall outside the anchored UTC window.`,
      'Verify ingestion timestamps and collection-window drift before using session counts as daily totals.',
      sessionSource.name,
    );

  const failedSessions = sessionRows.filter((row) =>
    ['failed', 'error', 'timeout'].includes(row.violation_status.trim().toLowerCase()),
  ).length;
  if (failedSessions)
    add(
      'attention',
      'Session processing failures observed',
      `${failedSessions} timestamp-eligible collected sessions explicitly report failed, error, or timeout status.`,
      'Review these sessions in SCM and check integration error handling. This is not a tenant-wide failure rate.',
      sessionSource.name,
    );
  const chartSessions =
    chartSource.status === 'unavailable' ? null : total(chart.map((bucket) => count(bucket.total)));
  const chartViolatingSessions =
    chartSource.status === 'unavailable'
      ? null
      : total(chart.map((bucket) => count(bucket.violating)));
  if (
    chartSessions !== null &&
    sessionSource.status === 'complete' &&
    chartSessions !== rawSessions.length
  )
    add(
      'review',
      'Session sources disagree',
      `The chart reports ${chartSessions} sessions; the independently paginated inventory returned ${rawSessions.length}.`,
      'Preserve both measurements and verify in SCM. Separate rolling queries are not a consistent snapshot; the discrepancy has no verified cause.',
      chartSource.name,
    );
  const urgentViolations = total(
    trend.map((bucket) =>
      total([count(bucket.violation_breakdown.critical), count(bucket.violation_breakdown.high)]),
    ),
  );
  if ((urgentViolations ?? 0) > 0)
    add(
      'attention',
      'High or critical detector violations observed',
      `${urgentViolations} high/critical detector violations in the returned daily trend. These are not distinct sessions.`,
      'Investigate the affected applications and confirm enforcement decisions before assigning incident severity.',
      trendSource.name,
    );
  const priorities = { attention: 0, review: 1, info: 2 };
  findings.sort((left, right) => priorities[left.priority] - priorities[right.priority]);

  return {
    schemaVersion: 2,
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
    health: sources.every((s) => s.status === 'unavailable')
      ? 'unknown'
      : findings.some((f) => f.priority === 'attention')
        ? 'attention'
        : findings.some((f) => f.priority === 'review')
          ? 'review'
          : 'no-findings',
    sources,
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
    sessions: {
      entries: sessionSource.status === 'unavailable' ? null : sessionRows.length,
      statuses: distribution(sessionRows),
      violatingSessions:
        sessionSource.status === 'unavailable'
          ? null
          : sessionRows.filter((row) => row.violation_status === 'violated').length,
      missingTimestamps,
      outsideWindow,
    },
    dailyTelemetry: {
      chart: {
        sessions: chartSessions,
        violatingSessions: chartViolatingSessions,
        buckets: chart.map((bucket) => ({
          time: label(bucket.time),
          sessions: count(bucket.total),
          violatingSessions: count(bucket.violating),
          violations: severity(bucket.violation_breakdown),
        })),
      },
      topApplications: rankings.map((app) => ({
        name: label(app.name),
        violations: count(app.total_violations),
        detectors: app.policy_violations.map((detector) => ({
          name: label(detector.detection_type),
          count: count(detector.total),
        })),
      })),
      violationTrend: trend.map((bucket) => ({
        time: label(bucket.date),
        violations: severity(bucket.violation_breakdown),
      })),
    },
    limitations: [
      'This is a read-only operational review, not an uptime SLA, compliance attestation, or proof that attacks succeeded or were blocked. No health score is invented.',
      'Application activity uses the API’s rolling one-day window; each paginated request evaluates its own server-relative window. Collection is not a transactionally consistent historical snapshot. Ingestion may lag.',
      'Application buckets, session inventory, chart sessions, and detector violations are different measurements. They are not added together. Session inventory summaries use timestamp-eligible entries only; absent data is unknown.',
      'Application buckets are keyed by registered application ID plus the literal scan metadata.app_name, which can differ from registered names. Do not sum session counts as unique users or unique tenant-wide sessions.',
      'Current configuration is not a configuration-change audit. Missing policy settings are unknown, not disabled. Inactive profiles can be intentional; current profile state may differ from the revision used by a historical scan.',
      'Daily charts, rankings and severity trends come from their own one-day endpoints. Rankings are a server-selected subset, not a complete inventory. Per-app token summaries and drill-downs require longer windows; no daily token usage or previous-day comparison is inferred.',
      'The legacy ScanLogsClient / scan-logs query path is broken and under refactor. This report uses the verified dashboard session APIs instead. It never automatically fetches transactions or stored scan content.',
      'Prompts, responses, user identities/IPs, API keys, auth codes, tenant IDs, and raw errors are omitted. Application/profile names and configuration metadata remain confidential; review before sharing.',
    ],
  };
}
