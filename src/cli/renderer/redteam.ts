import chalk from 'chalk';
import { dump as yamlDump } from 'js-yaml';
import { formatOutput, type OutputFormat } from './common.js';
import { ui } from './ui.js';

/** Render the red team banner. */
export function renderRedteamHeader(): void {
  ui.header('Prisma AIRS — AI Red Team', 'Adversarial scan operations');
}

type ChalkFn = (text: string) => string;

/** Severity → chalk color mapping (inline value coloring within composed lines). */
function severityColor(severity: string): ChalkFn {
  switch (severity.toUpperCase()) {
    case 'CRITICAL':
      return chalk.red;
    case 'HIGH':
      return chalk.magenta;
    case 'MEDIUM':
      return chalk.yellow;
    case 'LOW':
      return chalk.cyan;
    default:
      return chalk.dim;
  }
}

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

/** Colorize an active/inactive state word. */
function activeState(active: boolean): string {
  return statusColor(active ? 'COMPLETED' : 'FAILED')(active ? 'active' : 'inactive');
}

/** Render a scan's status summary. */
export function renderScanStatus(job: {
  uuid: string;
  name: string;
  status: string;
  jobType: string;
  targetName?: string;
  score?: number | null;
  asr?: number | null;
  completed?: number | null;
  total?: number | null;
}): void {
  ui.section('Scan Status:');
  const pairs: Array<[string, unknown]> = [
    ['ID', job.uuid],
    ['Name', job.name],
    ['Type', job.jobType],
  ];
  if (job.targetName) pairs.push(['Target', job.targetName]);
  pairs.push(['Status', statusColor(job.status)(job.status)]);
  if (job.total != null && job.completed != null) {
    pairs.push(['Progress', `${job.completed}/${job.total}`]);
  }
  if (job.score != null) pairs.push(['Score', job.score]);
  if (job.asr != null) pairs.push(['ASR', `${job.asr.toFixed(1)}%`]);
  ui.keyValue(pairs);
  console.log();
}

/** Render a table of scans. */
export function renderScanList(
  jobs: Array<{
    uuid: string;
    name: string;
    status: string;
    jobType: string;
    score?: number | null;
    createdAt?: string | null;
  }>,
  format: OutputFormat = 'pretty',
): void {
  if (jobs.length === 0) {
    ui.emptyList('scans');
    return;
  }
  if (format !== 'pretty') {
    const rows = jobs.map((j) => ({
      id: j.uuid,
      name: j.name,
      status: j.status,
      type: j.jobType,
      score: j.score ?? '',
      createdAt: j.createdAt ?? '',
    }));
    console.log(
      formatOutput(
        rows,
        [
          { key: 'id', label: 'ID' },
          { key: 'name', label: 'Name' },
          { key: 'status', label: 'Status' },
          { key: 'type', label: 'Type' },
          { key: 'score', label: 'Score' },
          { key: 'createdAt', label: 'Created' },
        ],
        format,
      ),
    );
    return;
  }
  ui.section('Recent Scans:');
  for (const job of jobs) {
    ui.dim(job.uuid);
    console.log(
      `    ${job.name}  ${statusColor(job.status)(job.status)}  ${job.jobType}${job.score != null ? `  score: ${job.score}` : ''}`,
    );
    if (job.createdAt) console.log(`    ${chalk.dim(job.createdAt)}`);
    console.log();
  }
}

/** Render a static scan report. */
export function renderStaticReport(report: {
  score?: number | null;
  asr?: number | null;
  severityBreakdown: Array<{ severity: string; successful: number; failed: number }>;
  reportSummary?: string | null;
  categories: Array<{
    id: string;
    displayName: string;
    asr: number;
    successful: number;
    failed: number;
    total: number;
  }>;
}): void {
  ui.section('Static Scan Report:');
  const pairs: Array<[string, unknown]> = [];
  if (report.score != null) pairs.push(['Score', report.score]);
  if (report.asr != null) pairs.push(['ASR', `${report.asr.toFixed(1)}%`]);
  if (pairs.length > 0) ui.keyValue(pairs);

  if (report.severityBreakdown.length > 0) {
    ui.section('Severity Breakdown:');
    for (const s of report.severityBreakdown) {
      const color = severityColor(s.severity);
      console.log(
        `    ${color(s.severity.padEnd(10))} ${chalk.red(`${s.successful} bypassed`)}  ${chalk.green(`${s.failed} blocked`)}`,
      );
    }
  }

  if (report.categories.length > 0) {
    ui.section('Categories:');
    ui.table(
      [
        { key: 'category', label: 'Category' },
        { key: 'asr', label: 'ASR' },
        { key: 'hits', label: 'Bypassed/Total' },
      ],
      report.categories.map((c) => ({
        category: c.displayName,
        asr: `${c.asr.toFixed(1)}%`,
        hits: `${c.successful}/${c.total}`,
      })),
    );
  }

  if (report.reportSummary) {
    ui.section('Summary:');
    console.log(`    ${report.reportSummary}`);
  }
  console.log();
}

/** Render a dynamic scan report. */
export function renderDynamicReport(report: {
  score?: number;
  asr?: number;
  totalGoals?: number;
  goalsAchieved?: number;
  totalStreams?: number;
  totalThreats?: number;
  reportSummary?: string | null;
}): void {
  ui.section('Dynamic Scan Report:');
  const pairs: Array<[string, unknown]> = [];
  if (report.score != null) pairs.push(['Score', report.score]);
  if (report.asr != null) pairs.push(['ASR', `${(report.asr * 100).toFixed(1)}%`]);
  if (report.totalGoals != null || report.goalsAchieved != null) {
    pairs.push([
      'Goals',
      `${report.goalsAchieved ?? 0} achieved / ${report.totalGoals ?? 0} total`,
    ]);
  }
  if (report.totalStreams != null) pairs.push(['Streams', report.totalStreams]);
  if (report.totalThreats != null) pairs.push(['Threats', report.totalThreats]);
  if (pairs.length > 0) ui.keyValue(pairs);

  if (report.reportSummary) {
    ui.section('Summary:');
    console.log(`    ${report.reportSummary}`);
  }
  console.log();
}

/** Render a custom attack report. */
export function renderCustomReport(report: {
  totalPrompts: number;
  totalAttacks: number;
  totalThreats: number;
  score: number;
  asr: number;
  promptSets: Array<{
    promptSetName: string;
    totalPrompts: number;
    totalThreats: number;
    threatRate: number;
  }>;
}): void {
  ui.section('Custom Attack Report:');
  ui.keyValue([
    ['Score', report.score],
    ['ASR', `${report.asr.toFixed(1)}%`],
    ['Attacks', report.totalAttacks],
    ['Threats', report.totalThreats],
  ]);

  if (report.promptSets.length > 0) {
    ui.section('Prompt Sets:');
    ui.table(
      [
        { key: 'promptSet', label: 'Prompt Set' },
        { key: 'threats', label: 'Threats' },
        { key: 'threatRate', label: 'Threat Rate' },
      ],
      report.promptSets.map((ps) => ({
        promptSet: ps.promptSetName,
        threats: `${ps.totalThreats}/${ps.totalPrompts}`,
        threatRate: `${ps.threatRate.toFixed(1)}%`,
      })),
    );
  }
  console.log();
}

/** Render attack list with severity coloring. */
export function renderAttackList(
  attacks: Array<{
    name?: string;
    severity?: string;
    category?: string;
    subCategory?: string;
    subCategoryDisplayName?: string;
    successful: boolean;
  }>,
  options?: { footnote?: string },
): void {
  if (attacks.length === 0) {
    ui.emptyList('attacks');
    if (options?.footnote) ui.dim(options.footnote);
    return;
  }
  ui.section('Attacks:');
  for (const a of attacks) {
    const sev = a.severity
      ? severityColor(a.severity)(a.severity.padEnd(10))
      : chalk.dim('N/A'.padEnd(10));
    const result = a.successful ? chalk.red('BYPASSED') : chalk.green('BLOCKED');
    const label = a.subCategoryDisplayName ?? a.subCategory ?? '—';
    console.log(`    ${sev} ${result}  ${label}${a.category ? chalk.dim(` [${a.category}]`) : ''}`);
  }
  if (options?.footnote) ui.dim(options.footnote);
  console.log();
}

/**
 * Build a footnote for `report --attacks --severity X` when the list-attacks
 * endpoint returns fewer items than the summary breakdown expects. Returns
 * undefined when no footnote is warranted.
 */
export function buildAttackListFootnote(args: {
  severity?: string;
  totalItems?: number;
  severityBreakdown: Array<{ severity: string; successful: number; failed: number }>;
}): string | undefined {
  if (!args.severity || args.totalItems == null) return undefined;
  const row = args.severityBreakdown.find((s) => s.severity === args.severity);
  if (!row) return undefined;
  const expected = row.successful + row.failed;
  if (args.totalItems >= expected) return undefined;
  return `(showing ${args.totalItems} of ${expected} expected for severity ${args.severity} — list-attacks endpoint excludes some variants; tracking upstream divergence at #206)`;
}

/** Render custom attack list (prompt-level results). */
export function renderCustomAttackList(
  attacks: Array<{
    promptText: string;
    goal?: string;
    threat: boolean;
    asr?: number;
    promptSetName?: string;
  }>,
): void {
  if (attacks.length === 0) {
    ui.emptyList('custom attacks');
    return;
  }
  ui.section('Custom Attacks:');
  for (const a of attacks) {
    const result = a.threat ? chalk.red('THREAT') : chalk.green('SAFE');
    const prompt = a.promptText.length > 80 ? `${a.promptText.substring(0, 77)}...` : a.promptText;
    const asrStr = a.asr != null ? chalk.dim(` ASR: ${a.asr.toFixed(1)}%`) : '';
    console.log(`    ${result}${asrStr}  ${prompt}`);
    if (a.goal) console.log(`      ${chalk.dim(a.goal)}`);
  }
  console.log();
}

/** Render target list. */
export function renderTargetList(
  targets: Array<{
    uuid: string;
    name: string;
    status: string;
    targetType?: string;
    active: boolean;
  }>,
  format: OutputFormat = 'pretty',
): void {
  if (targets.length === 0) {
    ui.emptyList('targets');
    return;
  }
  if (format !== 'pretty') {
    const rows = targets.map((t) => ({
      id: t.uuid,
      name: t.name,
      status: t.active ? 'active' : 'inactive',
      type: t.targetType ?? '',
    }));
    console.log(
      formatOutput(
        rows,
        [
          { key: 'id', label: 'ID' },
          { key: 'name', label: 'Name' },
          { key: 'status', label: 'Status' },
          { key: 'type', label: 'Type' },
        ],
        format,
      ),
    );
    return;
  }
  ui.section('Targets:');
  for (const t of targets) {
    ui.dim(t.uuid);
    console.log(
      `    ${t.name}  ${activeState(t.active)}${t.targetType ? `  type: ${t.targetType}` : ''}`,
    );
  }
  console.log();
}

/** Render attack category tree. */
export function renderCategories(
  categories: Array<{
    id: string;
    displayName: string;
    description?: string;
    subCategories: Array<{ id: string; displayName: string; description?: string }>;
  }>,
): void {
  if (categories.length === 0) {
    ui.emptyList('categories');
    return;
  }
  ui.section('Attack Categories:');
  for (const c of categories) {
    console.log(
      `  ${chalk.bold(c.displayName)} ${chalk.cyan(`(${c.id})`)}${c.description ? chalk.dim(` — ${c.description}`) : ''}`,
    );
    for (const sc of c.subCategories) {
      console.log(
        `    ${chalk.dim('•')} ${sc.displayName} ${chalk.cyan(`(${sc.id})`)}${sc.description ? chalk.dim(` — ${sc.description}`) : ''}`,
      );
    }
    console.log();
  }
}

/** Render prompt set list. */
export function renderPromptSetList(
  promptSets: Array<{ uuid: string; name: string; active: boolean }>,
  format: OutputFormat = 'pretty',
): void {
  if (promptSets.length === 0) {
    ui.emptyList('prompt sets');
    return;
  }
  if (format !== 'pretty') {
    const rows = promptSets.map((ps) => ({
      id: ps.uuid,
      name: ps.name,
      status: ps.active ? 'active' : 'inactive',
    }));
    console.log(
      formatOutput(
        rows,
        [
          { key: 'id', label: 'ID' },
          { key: 'name', label: 'Name' },
          { key: 'status', label: 'Status' },
        ],
        format,
      ),
    );
    return;
  }
  ui.section('Prompt Sets:');
  for (const ps of promptSets) {
    ui.dim(ps.uuid);
    console.log(`    ${ps.name}  ${activeState(ps.active)}`);
  }
  console.log();
}

/** Format a scalar/object value for pretty rendering — nested objects become indented JSON. */
function formatDetailValue(value: unknown, indent: string): string {
  if (value === null || value === undefined) return String(value);
  if (typeof value !== 'object') return String(value);
  const json = JSON.stringify(value, null, 2);
  // Indent every line after the first so the block lines up under the key.
  return json.split('\n').join(`\n${indent}`);
}

/** Render a key/value object block as an aligned keyValue list with JSON-expanded nested values. */
function keyValueFromObject(obj: Record<string, unknown>, skipNullish = false): void {
  const pairs: Array<[string, unknown]> = [];
  for (const [k, v] of Object.entries(obj)) {
    if (skipNullish && v == null) continue;
    pairs.push([k, formatDetailValue(v, '  ')]);
  }
  if (pairs.length > 0) ui.keyValue(pairs);
}

/** Render target detail. */
export function renderTargetDetail(
  target: {
    uuid: string;
    name: string;
    status: string;
    targetType?: string;
    active: boolean;
    connectionParams?: Record<string, unknown>;
    background?: Record<string, unknown>;
    metadata?: Record<string, unknown>;
  },
  format: OutputFormat = 'pretty',
): void {
  if (format !== 'pretty') {
    if (format === 'json') {
      console.log(JSON.stringify(target, null, 2));
    } else if (format === 'yaml') {
      console.log(yamlDump(target));
    }
    return;
  }
  ui.section('Target Detail:');
  const pairs: Array<[string, unknown]> = [
    ['UUID', target.uuid],
    ['Name', target.name],
    ['Status', activeState(target.active)],
  ];
  if (target.targetType) pairs.push(['Type', target.targetType]);
  ui.keyValue(pairs);
  if (target.connectionParams) {
    ui.section('Connection:');
    keyValueFromObject(target.connectionParams);
  }
  if (target.background) {
    ui.section('Background:');
    keyValueFromObject(target.background, true);
  }
  if (target.metadata) {
    ui.section('Metadata:');
    keyValueFromObject(target.metadata, true);
  }
  console.log();
}

/** Render prompt set detail. */
export function renderPromptSetDetail(
  ps: {
    uuid: string;
    name: string;
    active: boolean;
    archive: boolean;
    description?: string;
    createdAt?: string;
    updatedAt?: string;
  },
  format: OutputFormat = 'pretty',
  info?: {
    uuid: string;
    version: number;
    stats: { total: number; active: number; inactive: number };
  },
): void {
  if (format !== 'pretty') {
    const payload = info ? { ...ps, versionInfo: info } : { ...ps };
    if (format === 'json') {
      console.log(JSON.stringify(payload, null, 2));
    } else if (format === 'yaml') {
      console.log(yamlDump(payload));
    }
    return;
  }
  ui.section('Prompt Set Detail:');
  const pairs: Array<[string, unknown]> = [
    ['UUID', ps.uuid],
    ['Name', ps.name],
    ['Status', activeState(ps.active)],
    ['Archived', ps.archive ? 'yes' : 'no'],
  ];
  if (ps.description) pairs.push(['Description', ps.description]);
  if (ps.createdAt) pairs.push(['Created', ps.createdAt]);
  if (ps.updatedAt) pairs.push(['Updated', ps.updatedAt]);
  ui.keyValue(pairs);
  console.log();
}

/** Render prompt set version info. */
export function renderVersionInfo(info: {
  uuid: string;
  version: number;
  stats: { total: number; active: number; inactive: number };
}): void {
  ui.section('Version Info:');
  ui.keyValue([
    ['Version', info.version],
    ['Total', info.stats.total],
    ['Active', info.stats.active],
    ['Inactive', info.stats.inactive],
  ]);
  console.log();
}

/** Render a placeholder when the version-info endpoint is unavailable (upstream 500). */
export function renderVersionInfoUnavailable(): void {
  ui.section('Version Info:');
  ui.dim('unavailable (version-info endpoint returned an error)');
  console.log();
}

/** Render a list of prompts. */
export function renderPromptList(
  prompts: Array<{
    uuid: string;
    prompt: string;
    goal?: string;
    active: boolean;
  }>,
  format: OutputFormat = 'pretty',
): void {
  if (format !== 'pretty') {
    if (format === 'json') {
      console.log(JSON.stringify(prompts, null, 2));
    } else if (format === 'yaml') {
      console.log(yamlDump(prompts));
    }
    return;
  }
  if (prompts.length === 0) {
    ui.emptyList('prompts');
    return;
  }
  ui.section('Prompts:');
  for (const p of prompts) {
    const status = p.active ? chalk.green('active') : chalk.dim('inactive');
    const text = p.prompt.length > 80 ? `${p.prompt.substring(0, 77)}...` : p.prompt;
    console.log(`  ${chalk.dim(p.uuid)}  ${status}`);
    console.log(`    ${text}`);
    if (p.goal) console.log(`    ${chalk.dim(`Goal: ${p.goal}`)}`);
  }
  console.log();
}

/** Render prompt detail. */
export function renderPromptDetail(
  p: {
    uuid: string;
    prompt: string;
    goal?: string;
    active: boolean;
    promptSetId: string;
  },
  format: OutputFormat = 'pretty',
): void {
  if (format !== 'pretty') {
    if (format === 'json') {
      console.log(JSON.stringify(p, null, 2));
    } else if (format === 'yaml') {
      console.log(yamlDump(p));
    }
    return;
  }
  ui.section('Prompt Detail:');
  const pairs: Array<[string, unknown]> = [
    ['UUID', p.uuid],
    ['Set UUID', p.promptSetId],
    ['Status', p.active ? chalk.green('active') : chalk.dim('inactive')],
    ['Prompt', p.prompt],
  ];
  if (p.goal) pairs.push(['Goal', p.goal]);
  ui.keyValue(pairs);
  console.log();
}

/** Render property names list. */
export function renderPropertyNames(names: string[], format: OutputFormat = 'pretty'): void {
  if (format !== 'pretty') {
    if (format === 'json') {
      console.log(JSON.stringify(names, null, 2));
    } else if (format === 'yaml') {
      console.log(yamlDump(names));
    } else {
      const rows = names.map((n) => ({ name: n }));
      console.log(formatOutput(rows, [{ key: 'name', label: 'Name' }], format));
    }
    return;
  }
  if (names.length === 0) {
    ui.emptyList('property names');
    return;
  }
  ui.section('Property Names:');
  for (const n of names) {
    ui.bullet(n, 'neutral');
  }
  console.log();
}

/** Render target auth validation result. */
export function renderAuthValidation(result: {
  validated: boolean;
  tokenPreview?: string;
  expiresIn?: number;
}): void {
  ui.section('Auth Validation:');
  const pairs: Array<[string, unknown]> = [
    ['Validated', result.validated ? chalk.green('yes') : chalk.red('no')],
  ];
  if (result.tokenPreview) pairs.push(['Token', result.tokenPreview]);
  if (result.expiresIn != null) pairs.push(['Expires In', `${result.expiresIn}s`]);
  ui.keyValue(pairs);
  console.log();
}

/** Render target templates keyed by provider. */
export function renderTargetTemplates(templates: Record<string, unknown>): void {
  ui.section('Target Templates:');
  for (const [provider, config] of Object.entries(templates)) {
    ui.section(provider);
    ui.dim(JSON.stringify(config, null, 2).replace(/\n/g, '\n  '));
    console.log();
  }
}

/** Render EULA status. */
export function renderEulaStatus(status: {
  isAccepted: boolean;
  acceptedAt?: string;
  acceptedByUserId?: string;
}): void {
  ui.section('EULA Status:');
  const pairs: Array<[string, unknown]> = [
    ['Accepted', status.isAccepted ? chalk.green('yes') : chalk.red('no')],
  ];
  if (status.acceptedAt) pairs.push(['Accepted At', status.acceptedAt]);
  if (status.acceptedByUserId) pairs.push(['Accepted By', status.acceptedByUserId]);
  ui.keyValue(pairs);
  console.log();
}

/** Render EULA content. */
export function renderEulaContent(content: { content: string }): void {
  ui.section('EULA Content:');
  console.log(`  ${content.content}\n`);
}

/** Render property values for a single property name. */
export function renderPropertyValues(
  payload: { name: string; values: string[] },
  format: OutputFormat = 'pretty',
): void {
  if (format !== 'pretty') {
    if (format === 'json') {
      console.log(JSON.stringify(payload, null, 2));
    } else if (format === 'yaml') {
      console.log(yamlDump(payload));
    }
    return;
  }
  if (payload.values.length === 0) {
    ui.emptyList('property values');
    return;
  }
  ui.section('Property Values:');
  ui.keyValue([['Property', payload.name]]);
  for (const v of payload.values) {
    ui.bullet(v, 'neutral');
  }
  console.log();
}

/** Render instance response. */
export function renderInstanceResponse(resp: {
  tsgId: string;
  tenantId?: string;
  appId?: string;
  isSuccess?: boolean;
}): void {
  ui.section('Instance:');
  const pairs: Array<[string, unknown]> = [['TSG ID', resp.tsgId]];
  if (resp.tenantId) pairs.push(['Tenant ID', resp.tenantId]);
  if (resp.appId) pairs.push(['App ID', resp.appId]);
  if (resp.isSuccess != null) {
    pairs.push(['Success', resp.isSuccess ? chalk.green('yes') : chalk.red('no')]);
  }
  ui.keyValue(pairs);
  console.log();
}

/** Render instance detail (from GET). */
export function renderInstanceDetail(
  inst: {
    tsgId: string;
    tenantId: string;
    appId: string;
    region: string;
  },
  format: OutputFormat = 'pretty',
): void {
  if (format !== 'pretty') {
    if (format === 'json') {
      console.log(JSON.stringify(inst, null, 2));
    } else if (format === 'yaml') {
      console.log(yamlDump(inst));
    }
    return;
  }
  ui.section('Instance Detail:');
  ui.keyValue([
    ['TSG ID', inst.tsgId],
    ['Tenant ID', inst.tenantId],
    ['App ID', inst.appId],
    ['Region', inst.region],
  ]);
  console.log();
}

/** Render registry credentials. */
export function renderRegistryCredentials(
  creds: { token: string; expiry: string },
  format: OutputFormat = 'pretty',
): void {
  if (format !== 'pretty') {
    if (format === 'json') {
      console.log(JSON.stringify(creds, null, 2));
    } else if (format === 'yaml') {
      console.log(yamlDump(creds));
    }
    return;
  }
  ui.section('Registry Credentials:');
  ui.keyValue([
    ['Token', `${creds.token.substring(0, 20)}...`],
    ['Expiry', creds.expiry],
  ]);
  console.log();
}

/** Channel status → chalk color. */
function channelStatusColor(status: string): ChalkFn {
  switch (status.toUpperCase()) {
    case 'ONLINE':
      return chalk.green;
    case 'DRAFT':
      return chalk.yellow;
    case 'OFFLINE':
      return chalk.red;
    default:
      return chalk.white;
  }
}

/** Render a list of network broker channels. */
export function renderChannelList(
  channels: Array<{
    uuid?: string;
    name?: string | null;
    status?: string | null;
    connectedClientsCount?: number | null;
    lastOnlineAt?: string | null;
  }>,
  format: OutputFormat = 'pretty',
): void {
  if (channels.length === 0) {
    ui.emptyList('channels');
    return;
  }
  if (format !== 'pretty') {
    const rows = channels.map((c) => ({
      id: c.uuid ?? '',
      name: c.name ?? '',
      status: c.status ?? '',
      clients: c.connectedClientsCount ?? '',
      lastOnline: c.lastOnlineAt ?? '',
    }));
    console.log(
      formatOutput(
        rows,
        [
          { key: 'id', label: 'ID' },
          { key: 'name', label: 'Name' },
          { key: 'status', label: 'Status' },
          { key: 'clients', label: 'Clients' },
          { key: 'lastOnline', label: 'Last Online' },
        ],
        format,
      ),
    );
    return;
  }
  ui.section('Network Broker Channels:');
  for (const c of channels) {
    if (c.uuid) ui.dim(c.uuid);
    const status = c.status ? channelStatusColor(c.status)(c.status) : chalk.dim('unknown');
    const clients = c.connectedClientsCount != null ? `  clients: ${c.connectedClientsCount}` : '';
    console.log(`    ${c.name ?? '(unnamed)'}  ${status}${clients}`);
    console.log();
  }
}

/** Render a single channel's detail. */
export function renderChannelDetail(
  channel: {
    uuid?: string;
    name?: string | null;
    description?: string | null;
    status?: string | null;
    addedBy?: string | null;
    createdAt?: string | null;
    updatedAt?: string | null;
    lastOnlineAt?: string | null;
    connectedClientsCount?: number | null;
    outdatedClientsCount?: number | null;
    features?: Record<string, boolean> | null;
  },
  format: OutputFormat = 'pretty',
): void {
  if (format !== 'pretty') {
    console.log(format === 'json' ? JSON.stringify(channel, null, 2) : yamlDump(channel));
    return;
  }
  ui.section('Channel Detail:');
  const pairs: Array<[string, unknown]> = [['UUID', channel.uuid]];
  if (channel.name != null) pairs.push(['Name', channel.name]);
  if (channel.description != null) pairs.push(['Description', channel.description]);
  if (channel.status != null)
    pairs.push(['Status', channelStatusColor(channel.status)(channel.status)]);
  if (channel.connectedClientsCount != null)
    pairs.push(['Connected Clients', channel.connectedClientsCount]);
  if (channel.outdatedClientsCount != null)
    pairs.push(['Outdated Clients', channel.outdatedClientsCount]);
  if (channel.lastOnlineAt != null) pairs.push(['Last Online', channel.lastOnlineAt]);
  if (channel.addedBy != null) pairs.push(['Added By', channel.addedBy]);
  if (channel.createdAt != null) pairs.push(['Created', channel.createdAt]);
  if (channel.updatedAt != null) pairs.push(['Updated', channel.updatedAt]);
  ui.keyValue(pairs);
  if (channel.features && Object.keys(channel.features).length > 0) {
    ui.section('Features:');
    ui.keyValue(Object.entries(channel.features).map(([k, v]) => [k, v]));
  }
  console.log();
}

/** Render network broker channel statistics. */
export function renderChannelStats(
  stats: {
    serverDomain?: string | null;
    dockerRegistry?: string | null;
    helmChart?: string | null;
    dockerImage?: string | null;
    onlineChannels?: number | null;
    totalChannels?: number | null;
    clientVersion?: string | null;
  },
  format: OutputFormat = 'pretty',
): void {
  if (format !== 'pretty') {
    console.log(format === 'json' ? JSON.stringify(stats, null, 2) : yamlDump(stats));
    return;
  }
  ui.section('Network Broker Stats:');
  const pairs: Array<[string, unknown]> = [];
  if (stats.onlineChannels != null) pairs.push(['Online Channels', stats.onlineChannels]);
  if (stats.totalChannels != null) pairs.push(['Total Channels', stats.totalChannels]);
  if (stats.serverDomain != null) pairs.push(['Server Domain', stats.serverDomain]);
  if (stats.dockerRegistry != null) pairs.push(['Docker Registry', stats.dockerRegistry]);
  if (stats.dockerImage != null) pairs.push(['Docker Image', stats.dockerImage]);
  if (stats.helmChart != null) pairs.push(['Helm Chart', stats.helmChart]);
  if (stats.clientVersion != null) pairs.push(['Client Version', stats.clientVersion]);
  ui.keyValue(pairs);
  console.log();
}

/** Render tenant language configuration. */
export function renderLanguages(
  data: {
    multilingualEnabled: boolean;
    supportedJobTypes: string[];
    languages: Array<{ code: string; name: string }>;
  },
  format: OutputFormat = 'pretty',
): void {
  if (format !== 'pretty') {
    if (format === 'json' || format === 'yaml') {
      console.log(format === 'json' ? JSON.stringify(data, null, 2) : yamlDump(data));
      return;
    }
    console.log(
      formatOutput(
        data.languages.map((l) => ({ code: l.code, name: l.name })),
        [
          { key: 'code', label: 'Code' },
          { key: 'name', label: 'Name' },
        ],
        format,
      ),
    );
    return;
  }
  ui.section('Tenant Languages:');
  ui.keyValue([
    ['Multilingual', data.multilingualEnabled ? activeState(true) : activeState(false)],
    ['Supported Job Types', data.supportedJobTypes.join(', ') || '(none)'],
  ]);
  if (data.languages.length === 0) {
    ui.emptyList('languages');
    return;
  }
  ui.section('Languages:');
  for (const l of data.languages) {
    console.log(`    ${chalk.dim(l.code)}  ${l.name}`);
  }
  console.log();
}

/** Render target-profile error logs. */
export function renderErrorLogs(
  logs: Array<{
    createdAt: string;
    jobId?: string | null;
    attackId?: string | null;
    errorType?: string | null;
    errorSource?: string | null;
    errorMessage?: string | null;
  }>,
  format: OutputFormat = 'pretty',
): void {
  if (logs.length === 0) {
    ui.emptyList('error logs');
    return;
  }
  if (format !== 'pretty') {
    const rows = logs.map((l) => ({
      createdAt: l.createdAt,
      errorType: l.errorType ?? '',
      errorSource: l.errorSource ?? '',
      jobId: l.jobId ?? '',
      message: l.errorMessage ?? '',
    }));
    console.log(
      formatOutput(
        rows,
        [
          { key: 'createdAt', label: 'Created' },
          { key: 'errorType', label: 'Type' },
          { key: 'errorSource', label: 'Source' },
          { key: 'jobId', label: 'Job' },
          { key: 'message', label: 'Message' },
        ],
        format,
      ),
    );
    return;
  }
  ui.section('Target-Profile Error Logs:');
  for (const l of logs) {
    const type = l.errorType ? chalk.red(l.errorType) : chalk.dim('error');
    console.log(
      `    ${chalk.dim(l.createdAt)}  ${type}${l.errorSource ? `  (${l.errorSource})` : ''}`,
    );
    if (l.errorMessage) console.log(`      ${l.errorMessage}`);
    if (l.jobId) console.log(`      ${chalk.dim(`job: ${l.jobId}`)}`);
    console.log();
  }
}

/** Render an adapter list in the requested format. */
export function renderAdapterList(
  adapters: Array<{
    uuid: string;
    name: string;
    status: string;
    createdAt?: string;
    updatedAt?: string;
    targetCount?: number | null;
  }>,
  format: OutputFormat = 'pretty',
  totalItems?: number,
): void {
  if (adapters.length === 0) {
    ui.emptyList('adapters');
    return;
  }
  if (format !== 'pretty') {
    const rows = adapters.map((a) => ({
      uuid: a.uuid,
      name: a.name,
      status: a.status,
      targets: a.targetCount ?? '',
      updated: a.updatedAt ?? '',
    }));
    console.log(
      formatOutput(
        rows,
        [
          { key: 'uuid', label: 'UUID' },
          { key: 'name', label: 'Name' },
          { key: 'status', label: 'Status' },
          { key: 'targets', label: 'Targets' },
          { key: 'updated', label: 'Updated' },
        ],
        format,
      ),
    );
    return;
  }
  ui.section('Custom Target Adapters:');
  for (const a of adapters) {
    ui.dim(a.uuid);
    const status = a.status === 'ACTIVE' ? chalk.green(a.status) : chalk.yellow(a.status);
    const targets = a.targetCount != null ? `  targets: ${a.targetCount}` : '';
    console.log(`    ${a.name}  ${status}${targets}`);
    console.log();
  }
  if (totalItems !== undefined) ui.dim(`${totalItems} total`);
}

/**
 * Render an adapter detail. Secret values arrive masked (`'**********'`) with
 * `isRedacted: true` — render the flag, never pretend the mask is the value.
 */
export function renderAdapterDetail(
  adapter: {
    uuid: string;
    name: string;
    status: string;
    scriptB64: string;
    description?: string | null;
    networkBrokerChannelUuid?: string | null;
    variables: Array<{
      key: string;
      value?: string | null;
      type: 'VAR' | 'SECRET';
      isRedacted?: boolean;
    }>;
    targetCount?: number | null;
    createdAt?: string | null;
    updatedAt?: string | null;
  },
  format: OutputFormat = 'pretty',
): void {
  if (format !== 'pretty') {
    console.log(format === 'json' ? JSON.stringify(adapter, null, 2) : yamlDump(adapter));
    return;
  }
  ui.section('Adapter Detail:');
  const pairs: Array<[string, unknown]> = [
    ['UUID', adapter.uuid],
    ['Name', adapter.name],
    [
      'Status',
      adapter.status === 'ACTIVE' ? chalk.green(adapter.status) : chalk.yellow(adapter.status),
    ],
    ['Script', `${adapter.scriptB64.length} base64 chars`],
  ];
  if (adapter.description != null) pairs.push(['Description', adapter.description]);
  if (adapter.networkBrokerChannelUuid != null)
    pairs.push(['Broker Channel', adapter.networkBrokerChannelUuid]);
  if (adapter.targetCount != null) pairs.push(['Targets', adapter.targetCount]);
  if (adapter.createdAt != null) pairs.push(['Created', adapter.createdAt]);
  if (adapter.updatedAt != null) pairs.push(['Updated', adapter.updatedAt]);
  ui.keyValue(pairs);
  if (adapter.variables.length > 0) {
    ui.section('Variables:');
    ui.keyValue(
      adapter.variables.map((v) => [
        `${v.key} (${v.type})`,
        v.isRedacted ? chalk.dim('(redacted)') : (v.value ?? ''),
      ]),
    );
  }
  console.log();
}

/** Render a validation run outcome; on failure, stderr/traceback are the useful part. */
export function renderAdapterValidation(
  result: {
    validated: boolean;
    stdout?: string | null;
    stderr?: string | null;
    traceback?: string | null;
  },
  format: OutputFormat = 'pretty',
): void {
  if (format !== 'pretty') {
    console.log(format === 'json' ? JSON.stringify(result, null, 2) : yamlDump(result));
    return;
  }
  if (result.validated) {
    ui.success('Adapter script validated');
  } else {
    ui.error('Adapter script validation FAILED');
  }
  if (result.stdout) {
    ui.section('stdout:');
    console.log(result.stdout);
  }
  if (result.stderr) {
    ui.section('stderr:');
    console.log(chalk.red(result.stderr));
  }
  if (result.traceback) {
    ui.section('traceback:');
    console.log(chalk.red(result.traceback));
  }
  console.log();
}
