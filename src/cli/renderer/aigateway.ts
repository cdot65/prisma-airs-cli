import chalk from 'chalk';
import { dump as yamlDump } from 'js-yaml';
import type { AiGatewayWorkspace, AiGatewayWorkspaceDetail } from '../../airs/types.js';
import { formatOutput, type OutputFormat } from './common.js';
import { ui } from './ui.js';

/** Standard header for aigateway commands. */
export function renderAiGatewayHeader(): void {
  ui.header('Prisma AIRS — AI Gateway', 'Gateway workspace operations');
}

type ChalkFn = (text: string) => string;

function statusColor(status: string): ChalkFn {
  switch (status.toLowerCase()) {
    case 'active':
      return chalk.green;
    case 'archived':
      return chalk.yellow;
    default:
      return chalk.dim;
  }
}

/**
 * `get` can report a null status for a workspace `list` calls active — render
 * it as "unknown", never as inactive.
 */
function statusLabel(status: string | null | undefined): string {
  return status ?? 'unknown';
}

/** Render a workspace list in the requested format. */
export function renderWorkspaceList(
  workspaces: AiGatewayWorkspace[],
  format: OutputFormat = 'pretty',
): void {
  if (workspaces.length === 0) {
    ui.emptyList('workspaces');
    return;
  }
  if (format !== 'pretty') {
    const rows = workspaces.map((w) => ({
      id: w.id,
      slug: w.slug,
      name: w.name,
      status: statusLabel(w.status),
      isDefault: w.isDefault,
      scopeName: w.scopeName ?? '',
    }));
    console.log(
      formatOutput(
        rows,
        [
          { key: 'id', label: 'ID' },
          { key: 'slug', label: 'Slug' },
          { key: 'name', label: 'Name' },
          { key: 'status', label: 'Status' },
          { key: 'isDefault', label: 'Default' },
          { key: 'scopeName', label: 'Scope' },
        ],
        format,
      ),
    );
    return;
  }
  ui.section('AI Gateway Workspaces:');
  for (const w of workspaces) {
    ui.dim(w.id);
    const status = statusColor(statusLabel(w.status))(statusLabel(w.status));
    const dflt = w.isDefault ? chalk.cyan('  default') : '';
    console.log(`    ${w.name}  ${chalk.dim(w.slug)}  ${status}${dflt}`);
    if (w.scopeName) console.log(`    ${chalk.dim(`scope: ${w.scopeName}`)}`);
    console.log();
  }
}

/** Render a workspace detail in the requested format. */
export function renderWorkspaceDetail(
  workspace: AiGatewayWorkspaceDetail,
  format: OutputFormat = 'pretty',
): void {
  if (format !== 'pretty') {
    console.log(format === 'json' ? JSON.stringify(workspace, null, 2) : yamlDump(workspace));
    return;
  }
  ui.section('Workspace Detail:');
  const pairs: Array<[string, unknown]> = [
    ['ID', workspace.id],
    ['Slug', workspace.slug],
    ['Name', workspace.name],
    ['Status', statusColor(statusLabel(workspace.status))(statusLabel(workspace.status))],
    ['Default', workspace.isDefault ? 'yes' : 'no'],
  ];
  if (workspace.description != null) pairs.push(['Description', workspace.description]);
  if (workspace.scopeName != null) pairs.push(['Scope', workspace.scopeName]);
  if (workspace.createdAt != null) pairs.push(['Created', workspace.createdAt]);
  if (workspace.lastUpdatedAt != null) pairs.push(['Updated', workspace.lastUpdatedAt]);
  ui.keyValue(pairs);
  if (workspace.defaults && Object.keys(workspace.defaults).length > 0) {
    ui.section('Defaults:');
    console.log(chalk.dim(JSON.stringify(workspace.defaults, null, 2)));
  }
  if (workspace.usageLimits.length > 0) {
    ui.section('Usage Limits:');
    console.log(chalk.dim(JSON.stringify(workspace.usageLimits, null, 2)));
  }
  if (workspace.rateLimits.length > 0) {
    ui.section('Rate Limits:');
    console.log(chalk.dim(JSON.stringify(workspace.rateLimits, null, 2)));
  }
  if (workspace.securitySettings && Object.keys(workspace.securitySettings).length > 0) {
    ui.section('Security Settings:');
    ui.keyValue(Object.entries(workspace.securitySettings).map(([k, v]) => [k, v]));
  }
  console.log();
}

/**
 * Render a telemetry cost report. Every `*Cents` value is CENTS — the API
 * never converts. Pretty output shows dollars; structured output keeps the
 * raw cents fields so consumers are never handed a silently-scaled number.
 */
export function renderCostReport(
  report: {
    workspaceSlug: string;
    days: number;
    totalCents: number;
    avgCents: number;
    quotaExceeded: boolean;
    records: Array<{ date: string; costCents: number }>;
  },
  format: OutputFormat = 'pretty',
): void {
  if (format !== 'pretty') {
    console.log(format === 'json' ? JSON.stringify(report, null, 2) : yamlDump(report));
    return;
  }
  const dollars = (cents: number): string => `$${(cents / 100).toFixed(2)}`;
  ui.section(`Cost — ${report.workspaceSlug} (last ${report.days}d):`);
  ui.keyValue([
    ['Total', dollars(report.totalCents)],
    ['Daily average', dollars(report.avgCents)],
  ]);
  if (report.quotaExceeded) ui.warn('Telemetry quota exceeded — data may be truncated');
  if (report.records.length > 0) {
    ui.section('Per day:');
    ui.keyValue(report.records.map((r) => [r.date, dollars(r.costCents)]));
  }
  console.log();
}
