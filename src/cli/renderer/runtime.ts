import chalk from 'chalk';
import { formatOutput, type OutputFormat } from './common.js';

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
  console.log(
    chalk.dim(
      `  Tests: ${generated} generated, ${carriedFailures} carried failures, ${regressionTier} regression, ${total} total`,
    ),
  );
}

/** Render accumulated test count with optional dropped info. */
export function renderTestsAccumulated(
  newCount: number,
  totalCount: number,
  droppedCount: number,
): void {
  let msg = `  Tests: ${newCount} new, ${totalCount} total (accumulated)`;
  if (droppedCount > 0) {
    msg += chalk.yellow(` (${droppedCount} dropped by cap)`);
  }
  console.log(chalk.dim(msg));
}

type ChalkFn = (text: string) => string;

/** Status → chalk color mapping. */
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
  console.log(chalk.bold.cyan('\n  Prisma AIRS — Runtime Configuration'));
  console.log(chalk.dim('  Security profile and topic management\n'));
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
    console.log(chalk.dim('  No profiles found.\n'));
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

  console.log(chalk.bold('\n  Security Profiles:\n'));
  for (const p of profiles) {
    console.log(`  ${chalk.dim(p.profileId)}`);
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
  console.log(chalk.bold('\n  Profile Detail:\n'));
  console.log(`    ID:       ${chalk.dim(profile.profileId)}`);
  console.log(`    Name:     ${profile.profileName}`);
  console.log(`    Status:   ${profile.active ? chalk.green('active') : chalk.yellow('inactive')}`);
  if (profile.revision != null) console.log(`    Revision: ${profile.revision}`);
  if (profile.createdBy) console.log(`    Created:  ${chalk.dim(profile.createdBy)}`);
  if (profile.updatedBy) console.log(`    Updated:  ${chalk.dim(profile.updatedBy)}`);
  if (profile.lastModifiedTs) console.log(`    Modified: ${chalk.dim(profile.lastModifiedTs)}`);
  if (profile.policy) {
    const policyJson = JSON.stringify(profile.policy, null, 2);
    const indented = policyJson
      .split('\n')
      .map((line, i) => (i === 0 ? line : `              ${line}`))
      .join('\n');
    console.log(`    Policy:   ${chalk.dim(indented)}`);
  }
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
            revisions: g.keep.revision,
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

  console.log(chalk.bold('\n  Duplicate Profiles:\n'));

  const nameWidth = Math.max(7, ...groups.map((g) => g.name.length));
  const header = `  ${'Profile'.padEnd(nameWidth)}  Revisions  Keeping   Deleting`;
  console.log(chalk.dim(header));
  console.log(
    chalk.dim(`  ${'─'.repeat(nameWidth)}  ${'─'.repeat(9)}  ${'─'.repeat(8)}  ${'─'.repeat(8)}`),
  );

  for (const g of groups) {
    const total = g.remove.length + 1;
    console.log(
      `  ${g.name.padEnd(nameWidth)}  ${String(total).padStart(9)}  ${(`rev ${g.keep.revision}`).padStart(8)}  ${String(g.remove.length).padStart(8)}`,
    );
  }

  const totalRemove = groups.reduce((sum, g) => sum + g.remove.length, 0);
  console.log(
    `\n  Total: ${chalk.yellow(String(totalRemove))} old revisions to delete across ${groups.length} profiles\n`,
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
    console.log(chalk.bold.red('\n  Failures:\n'));
    for (const r of results.filter((r) => r.status === 'failed')) {
      console.log(`    ${chalk.red('✗')} ${r.name} rev ${r.revision}: ${r.error}`);
    }
  }

  const color = failed > 0 ? chalk.yellow : chalk.green;
  console.log(color(`\n  Cleanup complete: ${deleted} deleted, ${failed} failed\n`));
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
    console.log(chalk.dim('  No topics found.\n'));
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
  console.log(chalk.bold('\n  Custom Topics:\n'));
  for (const t of topics) {
    console.log(`  ${chalk.dim(t.topic_id)}`);
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
  console.log(chalk.bold('\n  Topic Detail:\n'));
  console.log(`    ID:          ${chalk.dim(topic.topic_id)}`);
  console.log(`    Name:        ${topic.topic_name}`);
  if (topic.revision != null) console.log(`    Revision:    ${topic.revision}`);
  if (topic.description) console.log(`    Description: ${topic.description}`);
  if (topic.examples?.length) {
    console.log('    Examples:');
    for (const ex of topic.examples) {
      console.log(`      ${chalk.dim('•')} ${ex}`);
    }
  }
  if (topic.created_by) console.log(`    Created:     ${chalk.dim(topic.created_by)}`);
  if (topic.updated_by) console.log(`    Updated:     ${chalk.dim(topic.updated_by)}`);
  if (topic.last_modified_ts) console.log(`    Modified:    ${chalk.dim(topic.last_modified_ts)}`);
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
    console.log(chalk.dim('  No API keys found.\n'));
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
  console.log(chalk.bold('\n  API Keys:\n'));
  for (const k of keys) {
    console.log(`  ${chalk.dim(k.id)}`);
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
  console.log(chalk.bold('\n  API Key Detail:\n'));
  console.log(`    ID:      ${chalk.dim(key.id)}`);
  console.log(`    Name:    ${key.name}`);
  if (key.apiKey) console.log(`    Key:     ${key.apiKey}`);
  else if (key.last8) console.log(`    Key:     ${chalk.dim('…')}${key.last8}`);
  if (key.createdAt) console.log(`    Created: ${chalk.dim(key.createdAt)}`);
  if (key.expiresAt) console.log(`    Expires: ${chalk.dim(key.expiresAt)}`);
  console.log();
}

/** Render customer app list. */
export function renderCustomerAppList(
  apps: Array<{ id?: string; name: string; description?: string }>,
  format: OutputFormat = 'pretty',
): void {
  if (apps.length === 0) {
    console.log(chalk.dim('  No customer apps found.\n'));
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
  console.log(chalk.bold('\n  Customer Apps:\n'));
  for (const a of apps) {
    if (a.id) console.log(`  ${chalk.dim(a.id)}`);
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
  console.log(chalk.bold('\n  Customer App Detail:\n'));
  if (app.id) console.log(`    ID:   ${chalk.dim(app.id)}`);
  console.log(`    Name: ${app.name}`);
  if (app.description) console.log(`    Desc: ${app.description}`);
  console.log(`    Data: ${chalk.dim(JSON.stringify(app.raw, null, 2).slice(0, 500))}`);
  console.log();
}

/** Render deployment profile list. */
export function renderDeploymentProfileList(
  profiles: Array<{ raw: Record<string, unknown> }>,
  format: OutputFormat = 'pretty',
): void {
  if (profiles.length === 0) {
    console.log(chalk.dim('  No deployment profiles found.\n'));
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
  console.log(chalk.bold('\n  Deployment Profiles:\n'));
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

/** Render DLP profile list. */
export function renderDlpProfileList(
  profiles: Array<{ raw: Record<string, unknown> }>,
  format: OutputFormat = 'pretty',
): void {
  if (profiles.length === 0) {
    console.log(chalk.dim('  No DLP profiles found.\n'));
    return;
  }
  if (format !== 'pretty') {
    const rows = profiles.map((p) => ({
      id: (p.raw.uuid ?? '') as string,
      name: (p.raw.name ?? p.raw.profile_name ?? '') as string,
    }));
    console.log(
      formatOutput(
        rows,
        [
          { key: 'id', label: 'ID' },
          { key: 'name', label: 'Name' },
        ],
        format,
      ),
    );
    return;
  }
  console.log(chalk.bold('\n  DLP Profiles:\n'));
  for (const p of profiles) {
    const name = (p.raw.name ?? p.raw.profile_name ?? 'unknown') as string;
    const uuid = (p.raw.uuid ?? '') as string;
    if (uuid) console.log(`  ${chalk.dim(uuid)}`);
    console.log(`    ${name}`);
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
    console.log(chalk.dim('  No scan logs found.\n'));
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
  console.log(chalk.bold(`\n  Scan Logs (${results.length} results):\n`));
  for (const r of results) {
    const action = (r.action ?? r.verdict) as string | undefined;
    const app = r.app_name as string | undefined;
    const profile = r.profile_name as string | undefined;
    const ts = (r.received_ts ?? r.timestamp) as string | undefined;
    const scanId = r.scan_id as string | undefined;
    const actionColor = action === 'block' ? chalk.red : chalk.green;
    if (scanId) console.log(`  ${chalk.dim(scanId)}`);
    console.log(
      `    ${ts ? chalk.dim(ts) : ''}  ${action ? actionColor(action) : ''}  ${profile ? `[${profile}]` : ''}  ${app ?? ''}`,
    );
  }
  if (pageToken) {
    console.log(chalk.dim(`\n  Page token: ${pageToken}`));
  }
  console.log();
}
