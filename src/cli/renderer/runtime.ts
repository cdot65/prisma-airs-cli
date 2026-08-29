import chalk from 'chalk';
import { formatOutput, type OutputFormat } from './common.js';
import { ui } from './ui.js';

/** Render polling progress inline. */
export function renderScanProgress(job: {
  status: string;
  completed?: number | null;
  total?: number | null;
}): void {
  if (job.total != null && job.completed != null && job.total > 0) {
    const pct = Math.round((job.completed / job.total) * 100);
    const bar = '█'.repeat(Math.round(pct / 5)) + '░'.repeat(20 - Math.round(pct / 5));
    process.stdout.write(
      `\r  ${statusColor(job.status)(job.status)} ${bar} ${pct}% (${job.completed}/${job.total})`,
    );
  } else {
    process.stdout.write(`\r  ${statusColor(job.status)(job.status)}...`);
  }
}

/** Render test composition summary (carried failures + regression + generated). */
export function renderTestsComposed(
  generated: number,
  carriedFailures: number,
  regressionTier: number,
  total: number,
): void {
  ui.dim(
    `Tests: ${generated} generated, ${carriedFailures} carried failures, ${regressionTier} regression, ${total} total`,
  );
}

/** Render accumulated test count with optional dropped info. */
export function renderTestsAccumulated(
  newCount: number,
  totalCount: number,
  droppedCount: number,
): void {
  let msg = `Tests: ${newCount} new, ${totalCount} total (accumulated)`;
  if (droppedCount > 0) {
    msg += ` (${droppedCount} dropped by cap)`;
  }
  ui.dim(msg);
}

type ChalkFn = (text: string) => string;

/** Status → chalk color mapping (inline value coloring within composed lines). */
function statusColor(status: string): ChalkFn {
  switch (status) {
    case 'COMPLETED':
      return chalk.green;
    case 'RUNNING':
      return chalk.blue;
    case 'QUEUED':
    case 'INIT':
      return chalk.yellow;
    case 'FAILED':
    case 'ABORTED':
      return chalk.red;
    case 'PARTIALLY_COMPLETE':
      return chalk.yellow;
    default:
      return chalk.white;
  }
}

// ---------------------------------------------------------------------------
// Runtime Config rendering — profiles, topics, api keys, etc.
// ---------------------------------------------------------------------------

/** Render the runtime config banner. */
export function renderRuntimeConfigHeader(): void {
  ui.header('Prisma AIRS — Runtime Configuration', 'Security profile and topic management');
}

/** Render security profile list. */
export function renderProfileList(
  profiles: Array<{
    profileId: string;
    profileName: string;
    revision?: number;
    active?: boolean;
    lastModifiedTs?: string;
  }>,
  format: OutputFormat = 'pretty',
): void {
  if (profiles.length === 0) {
    ui.emptyList('profiles');
    return;
  }

  if (format !== 'pretty') {
    const rows = profiles.map((p) => ({
      id: p.profileId,
      name: p.profileName,
      status: p.active ? 'active' : 'inactive',
      revision: p.revision ?? '',
    }));
    const columns = [
      { key: 'id', label: 'ID' },
      { key: 'name', label: 'Name' },
      { key: 'status', label: 'Status' },
      { key: 'revision', label: 'Revision' },
    ];
    console.log(formatOutput(rows, columns, format));
    return;
  }

  ui.section('Security Profiles:');
  for (const p of profiles) {
    ui.dim(p.profileId);
    const status = p.active ? chalk.green('active') : chalk.yellow('inactive');
    const rev = p.revision != null ? chalk.dim(` rev:${p.revision}`) : '';
    console.log(`    ${p.profileName}  ${status}${rev}`);
  }
  console.log();
}

/** Render security profile detail. */
export function renderProfileDetail(profile: {
  profileId: string;
  profileName: string;
  revision?: number;
  active?: boolean;
  createdBy?: string;
  updatedBy?: string;
  lastModifiedTs?: string;
  policy?: Record<string, unknown>;
}): void {
  ui.section('Profile Detail:');
  const pairs: Array<[string, unknown]> = [
    ['ID', profile.profileId],
    ['Name', profile.profileName],
    ['Status', profile.active ? chalk.green('active') : chalk.yellow('inactive')],
  ];
  if (profile.revision != null) pairs.push(['Revision', profile.revision]);
  if (profile.createdBy) pairs.push(['Created', profile.createdBy]);
  if (profile.updatedBy) pairs.push(['Updated', profile.updatedBy]);
  if (profile.lastModifiedTs) pairs.push(['Modified', profile.lastModifiedTs]);
  if (profile.policy) {
    const policyJson = JSON.stringify(profile.policy, null, 2);
    pairs.push(['Policy', policyJson.split('\n').join('\n  ')]);
  }
  ui.keyValue(pairs);
  console.log();
}

/** Render cleanup preview showing duplicate groups. */
export function renderCleanupPreview(
  groups: Array<{
    name: string;
    keep: { id: string; revision: number };
    remove: Array<{ id: string; revision: number }>;
  }>,
  format: 'pretty' | 'json' = 'pretty',
): void {
  if (format === 'json') {
    console.log(
      JSON.stringify(
        {
          duplicates: groups.map((g) => ({
            name: g.name,
            revisions: g.remove.length + 1,
            keeping: g.keep.revision,
            deleting: g.remove.length,
          })),
          total: groups.reduce((sum, g) => sum + g.remove.length, 0),
        },
        null,
        2,
      ),
    );
    return;
  }

  ui.section('Duplicate Profiles:');
  ui.table(
    [
      { key: 'profile', label: 'Profile' },
      { key: 'revisions', label: 'Revisions' },
      { key: 'keeping', label: 'Keeping' },
      { key: 'deleting', label: 'Deleting' },
    ],
    groups.map((g) => ({
      profile: g.name,
      revisions: g.remove.length + 1,
      keeping: `rev ${g.keep.revision}`,
      deleting: g.remove.length,
    })),
  );

  const totalRemove = groups.reduce((sum, g) => sum + g.remove.length, 0);
  console.log(
    `\n  Total: ${totalRemove} old revisions to delete across ${groups.length} profiles\n`,
  );
}

/** Render cleanup deletion results. */
export function renderCleanupResult(
  results: Array<{
    id: string;
    revision: number;
    name: string;
    status: 'ok' | 'failed';
    error?: string;
  }>,
  format: 'pretty' | 'json' = 'pretty',
): void {
  const deleted = results.filter((r) => r.status === 'ok').length;
  const failed = results.filter((r) => r.status === 'failed').length;

  if (format === 'json') {
    console.log(JSON.stringify({ deleted, failed, details: results }, null, 2));
    return;
  }

  if (failed > 0) {
    ui.section('Failures:');
    for (const r of results.filter((r) => r.status === 'failed')) {
      ui.bullet(`${r.name} rev ${r.revision}: ${r.error}`, 'error');
    }
  }

  console.log();
  if (failed > 0) {
    ui.warn(`Cleanup complete: ${deleted} deleted, ${failed} failed`);
  } else {
    ui.success(`Cleanup complete: ${deleted} deleted, ${failed} failed`);
  }
  console.log();
}

/** Render custom topic list. */
export function renderTopicList(
  topics: Array<{
    topic_id?: string;
    topic_name: string;
    description?: string;
    revision?: number;
  }>,
  format: OutputFormat = 'pretty',
): void {
  if (topics.length === 0) {
    ui.emptyList('topics');
    return;
  }
  if (format !== 'pretty') {
    const rows = topics.map((t) => ({
      id: t.topic_id ?? '',
      name: t.topic_name,
      revision: t.revision ?? '',
      description: t.description ?? '',
    }));
    console.log(
      formatOutput(
        rows,
        [
          { key: 'id', label: 'ID' },
          { key: 'name', label: 'Name' },
          { key: 'revision', label: 'Revision' },
          { key: 'description', label: 'Description' },
        ],
        format,
      ),
    );
    return;
  }
  ui.section('Custom Topics:');
  for (const t of topics) {
    ui.dim(String(t.topic_id));
    const rev = t.revision != null ? chalk.dim(` rev:${t.revision}`) : '';
    const desc = t.description ? chalk.dim(` — ${t.description.slice(0, 80)}`) : '';
    console.log(`    ${t.topic_name}${rev}${desc}`);
  }
  console.log();
}

/** Render custom topic detail. */
export function renderTopicDetail(topic: {
  topic_id?: string;
  topic_name: string;
  description?: string;
  examples?: string[];
  revision?: number;
  created_by?: string;
  updated_by?: string;
  last_modified_ts?: string;
}): void {
  ui.section('Topic Detail:');
  const pairs: Array<[string, unknown]> = [
    ['ID', topic.topic_id],
    ['Name', topic.topic_name],
  ];
  if (topic.revision != null) pairs.push(['Revision', topic.revision]);
  if (topic.description) pairs.push(['Description', topic.description]);
  ui.keyValue(pairs);
  if (topic.examples?.length) {
    console.log('  Examples:');
    for (const ex of topic.examples) {
      ui.bullet(ex, 'neutral');
    }
  }
  const metaPairs: Array<[string, unknown]> = [];
  if (topic.created_by) metaPairs.push(['Created', topic.created_by]);
  if (topic.updated_by) metaPairs.push(['Updated', topic.updated_by]);
  if (topic.last_modified_ts) metaPairs.push(['Modified', topic.last_modified_ts]);
  if (metaPairs.length > 0) ui.keyValue(metaPairs);
  console.log();
}

/** Render API key list. */
export function renderApiKeyList(
  keys: Array<{
    id: string;
    name: string;
    last8?: string;
    createdAt?: string;
    expiresAt?: string;
  }>,
  format: OutputFormat = 'pretty',
): void {
  if (keys.length === 0) {
    ui.emptyList('API keys');
    return;
  }
  if (format !== 'pretty') {
    const rows = keys.map((k) => ({
      id: k.id,
      name: k.name,
      last8: k.last8 ?? '',
      createdAt: k.createdAt ?? '',
      expiresAt: k.expiresAt ?? '',
    }));
    console.log(
      formatOutput(
        rows,
        [
          { key: 'id', label: 'ID' },
          { key: 'name', label: 'Name' },
          { key: 'last8', label: 'Key (last 8)' },
          { key: 'createdAt', label: 'Created' },
          { key: 'expiresAt', label: 'Expires' },
        ],
        format,
      ),
    );
    return;
  }
  ui.section('API Keys:');
  for (const k of keys) {
    ui.dim(k.id);
    const last8 = k.last8 ? chalk.dim(` key: …${k.last8}`) : '';
    const expires = k.expiresAt ? chalk.dim(` expires: ${k.expiresAt}`) : '';
    console.log(`    ${k.name}${last8}${expires}`);
  }
  console.log();
}

/** Render API key detail. */
export function renderApiKeyDetail(key: {
  id: string;
  name: string;
  apiKey?: string;
  last8?: string;
  createdAt?: string;
  expiresAt?: string;
}): void {
  ui.section('API Key Detail:');
  const pairs: Array<[string, unknown]> = [
    ['ID', key.id],
    ['Name', key.name],
  ];
  if (key.apiKey) pairs.push(['Key', key.apiKey]);
  else if (key.last8) pairs.push(['Key', `…${key.last8}`]);
  if (key.createdAt) pairs.push(['Created', key.createdAt]);
  if (key.expiresAt) pairs.push(['Expires', key.expiresAt]);
  ui.keyValue(pairs);
  console.log();
}

/** Render customer app list. */
export function renderCustomerAppList(
  apps: Array<{ id?: string; name: string; description?: string }>,
  format: OutputFormat = 'pretty',
): void {
  if (apps.length === 0) {
    ui.emptyList('customer apps');
    return;
  }
  if (format !== 'pretty') {
    const rows = apps.map((a) => ({
      id: a.id ?? '',
      name: a.name,
      description: a.description ?? '',
    }));
    console.log(
      formatOutput(
        rows,
        [
          { key: 'id', label: 'ID' },
          { key: 'name', label: 'Name' },
          { key: 'description', label: 'Description' },
        ],
        format,
      ),
    );
    return;
  }
  ui.section('Customer Apps:');
  for (const a of apps) {
    if (a.id) ui.dim(a.id);
    const desc = a.description ? chalk.dim(` — ${a.description.slice(0, 80)}`) : '';
    console.log(`    ${a.name}${desc}`);
  }
  console.log();
}

/** Render customer app detail. */
export function renderCustomerAppDetail(app: {
  id?: string;
  name: string;
  description?: string;
  raw: Record<string, unknown>;
}): void {
  ui.section('Customer App Detail:');
  const pairs: Array<[string, unknown]> = [];
  if (app.id) pairs.push(['ID', app.id]);
  pairs.push(['Name', app.name]);
  if (app.description) pairs.push(['Desc', app.description]);
  pairs.push(['Data', JSON.stringify(app.raw, null, 2).slice(0, 500)]);
  ui.keyValue(pairs);
  console.log();
}

/** Render per-app consumption + violation breakdown. */
export function renderCustomerAppConsumption(
  data: {
    appId: string;
    appName: string;
    cloud?: string;
    source?: string;
    monitoringSince?: string;
    profiles: string[];
    tokens: {
      dailyAverage?: number;
      dailyAverageScale?: string;
      monthlyTotal?: number;
      monthlyTotalScale?: string;
    };
    sessions: { total: number; violating: number };
    detectors: Array<{
      type: string;
      critical: number;
      high: number;
      medium: number;
      low: number;
      total: number;
    }>;
    totalViolating: number;
  },
  format: OutputFormat = 'pretty',
): void {
  const fmt = (n?: number, scale?: string) => (n == null ? '-' : `${n}${scale ?? ''}`);

  if (format !== 'pretty') {
    // Per-detector rows for table/csv/json/yaml. Adds app-level fields so each row is
    // self-contained (useful for piping into reporting tools).
    const rows = data.detectors.map((d) => ({
      app_name: data.appName,
      app_id: data.appId,
      monitoring_since: data.monitoringSince ?? '',
      daily_avg: fmt(data.tokens.dailyAverage, data.tokens.dailyAverageScale),
      monthly_total: fmt(data.tokens.monthlyTotal, data.tokens.monthlyTotalScale),
      sessions_total: data.sessions.total,
      sessions_violating: data.sessions.violating,
      detector: d.type,
      critical: d.critical,
      high: d.high,
      medium: d.medium,
      low: d.low,
      total: d.total,
    }));
    console.log(
      formatOutput(
        rows,
        [
          { key: 'app_name', label: 'App' },
          { key: 'app_id', label: 'AppId' },
          { key: 'monitoring_since', label: 'MonitoringSince' },
          { key: 'daily_avg', label: 'DailyAvg' },
          { key: 'monthly_total', label: 'MonthlyTotal' },
          { key: 'sessions_total', label: 'Sessions' },
          { key: 'sessions_violating', label: 'Violating' },
          { key: 'detector', label: 'Detector' },
          { key: 'critical', label: 'C' },
          { key: 'high', label: 'H' },
          { key: 'medium', label: 'M' },
          { key: 'low', label: 'L' },
          { key: 'total', label: 'Total' },
        ],
        format,
      ),
    );
    return;
  }

  ui.header(data.appName, `(${data.appId})`);
  const pairs: Array<[string, unknown]> = [];
  if (data.monitoringSince) pairs.push(['Monitoring since', data.monitoringSince]);
  if (data.source) pairs.push(['Source', data.source]);
  if (data.cloud) pairs.push(['Cloud', data.cloud]);
  if (data.profiles.length > 0) pairs.push(['Profiles', data.profiles.join(', ')]);
  if (pairs.length > 0) ui.keyValue(pairs);

  ui.section('Token consumption:');
  ui.keyValue([
    ['Daily avg', fmt(data.tokens.dailyAverage, data.tokens.dailyAverageScale)],
    ['Monthly total', fmt(data.tokens.monthlyTotal, data.tokens.monthlyTotalScale)],
  ]);

  ui.section('Sessions:');
  ui.keyValue([
    ['Total', data.sessions.total],
    ['Violating', data.sessions.violating],
  ]);

  const firing = data.detectors.filter((d) => d.total > 0);
  ui.section(
    `Detectors (${data.totalViolating} violating, ${firing.length}/${data.detectors.length} firing):`,
  );
  if (firing.length === 0) {
    ui.dim('no detector violations in window');
  } else {
    ui.table(
      [
        { key: 'detector', label: 'Detector' },
        { key: 'total', label: 'Total' },
        { key: 'severity', label: 'Severity' },
      ],
      firing.map((d) => ({
        detector: d.type,
        total: d.total,
        severity: `c=${d.critical} h=${d.high} m=${d.medium} l=${d.low}`,
      })),
    );
  }
  console.log();
}

/** Render deployment profile list. */
export function renderDeploymentProfileList(
  profiles: Array<{ raw: Record<string, unknown> }>,
  format: OutputFormat = 'pretty',
): void {
  if (profiles.length === 0) {
    ui.emptyList('deployment profiles');
    return;
  }
  if (format !== 'pretty') {
    const rows = profiles.map((p) => ({
      name: (p.raw.dp_name ?? p.raw.profile_name ?? p.raw.name ?? '') as string,
      status: (p.raw.status ?? '') as string,
      authCode: (p.raw.auth_code ?? '') as string,
    }));
    console.log(
      formatOutput(
        rows,
        [
          { key: 'name', label: 'Name' },
          { key: 'status', label: 'Status' },
          { key: 'authCode', label: 'Auth Code' },
        ],
        format,
      ),
    );
    return;
  }
  ui.section('Deployment Profiles:');
  for (const p of profiles) {
    const name = (p.raw.dp_name ?? p.raw.profile_name ?? p.raw.name ?? 'unknown') as string;
    const status = p.raw.status as string | undefined;
    const authCode = p.raw.auth_code as string | undefined;
    const statusColor = status === 'active' ? chalk.green : chalk.dim;
    console.log(
      `    ${name}${status ? `  ${statusColor(status)}` : ''}${authCode ? `  ${chalk.dim(authCode)}` : ''}`,
    );
  }
  console.log();
}

/** Render scan log results. */
export function renderScanLogList(
  results: Record<string, unknown>[],
  pageToken?: string,
  format: OutputFormat = 'pretty',
): void {
  if (results.length === 0) {
    if (format === 'pretty') ui.emptyList('scan logs');
    else console.log(formatOutput([], [], format));
    return;
  }
  if (format !== 'pretty') {
    const rows = results.map((r) => ({
      scanId: (r.scan_id ?? '') as string,
      timestamp: (r.received_ts ?? r.timestamp ?? '') as string,
      action: (r.action ?? r.verdict ?? '') as string,
      profile: (r.profile_name ?? '') as string,
      app: (r.app_name ?? '') as string,
    }));
    console.log(
      formatOutput(
        rows,
        [
          { key: 'scanId', label: 'Scan ID' },
          { key: 'timestamp', label: 'Timestamp' },
          { key: 'action', label: 'Action' },
          { key: 'profile', label: 'Profile' },
          { key: 'app', label: 'App' },
        ],
        format,
      ),
    );
    return;
  }
  ui.section(`Scan Logs (${results.length} results):`);
  for (const r of results) {
    const action = (r.action ?? r.verdict) as string | undefined;
    const app = r.app_name as string | undefined;
    const profile = r.profile_name as string | undefined;
    const ts = (r.received_ts ?? r.timestamp) as string | undefined;
    const scanId = r.scan_id as string | undefined;
    const actionColor = action === 'block' ? chalk.red : chalk.green;
    if (scanId) ui.dim(scanId);
    console.log(
      `    ${ts ? chalk.dim(ts) : ''}  ${action ? actionColor(action) : ''}  ${profile ? `[${profile}]` : ''}  ${app ?? ''}`,
    );
  }
  if (pageToken) {
    console.log();
    ui.dim(`Page token: ${pageToken}`);
  }
  console.log();
}
