import {
  AI_GW_GROUP_COLUMNS,
  AI_GW_GROUP_DIMENSIONS,
  type AIGatewayChartOptions,
  type AIGatewayGroupOptions,
  type AIGatewayWindowOptions,
} from '@cdot65/prisma-airs-sdk';
import type { Command } from 'commander';
import { CliUsageError } from '../../renderer/index.js';
import { addReadOutput, failAiGateway, runDetail, showHelpOnEmpty } from './shared.js';
import { parseDateOption, parseIntegerOption } from './structured-input.js';
import {
  addChartFilterOptions,
  type ChartFilterFlags,
  chartFiltersFrom,
} from './telemetry-filters.js';

interface WindowFlags {
  columns?: string;
  days?: string;
  end?: string;
  output?: string;
  start?: string;
  statusCode?: string;
  traceId?: string;
  workspace: string;
}

function addWindowOptions(command: Command): Command {
  return addReadOutput(
    command
      .requiredOption('--workspace <slug>', 'Workspace slug')
      .option('--days <n>', 'Rolling window in days', '7')
      .option('--end <iso>', 'Window end as ISO-8601')
      .option('--start <iso>', 'Window start as ISO-8601'),
  );
}

function windowFrom(opts: WindowFlags): AIGatewayWindowOptions {
  const window: AIGatewayWindowOptions = { workspaceSlug: opts.workspace };
  if (opts.start) window.start = parseNamedDate(opts.start, '--start');
  else window.days = parsePositiveInteger(opts.days ?? '7', '--days');
  if (opts.end) window.end = parseNamedDate(opts.end, '--end');
  return window;
}

export function parsePositiveInteger(value: unknown, flag: string): number {
  try {
    const parsed = parseIntegerOption(value);
    if (parsed <= 0) throw new CliUsageError('Expected a positive integer');
    return parsed;
  } catch (error) {
    if (error instanceof CliUsageError)
      throw new CliUsageError(`Invalid ${flag}: ${error.message}`);
    throw error;
  }
}

function parseNamedDate(value: unknown, flag: string): Date {
  try {
    return parseDateOption(value);
  } catch (error) {
    if (error instanceof CliUsageError)
      throw new CliUsageError(`Invalid ${flag}: ${error.message}`);
    throw error;
  }
}

function registerMetric(
  telemetry: Command,
  name: string,
  description: string,
  method: 'errorTrends' | 'errors' | 'rescuedRetries' | 'userTrends' | 'users',
): void {
  const command = addWindowOptions(telemetry.command(name).description(description));
  command.action((opts: WindowFlags) =>
    runDetail(command, opts, (client) => client.telemetry[method](windowFrom(opts))),
  );
}

function registerFilteredMetric(
  telemetry: Command,
  name: 'latency' | 'requests' | 'tokens',
  description: string,
): void {
  const command = addChartFilterOptions(
    addWindowOptions(telemetry.command(name).description(description)),
  );
  command.action(async (opts: WindowFlags & ChartFilterFlags) => {
    let options: AIGatewayChartOptions;
    try {
      options = { ...windowFrom(opts), ...chartFiltersFrom(opts) };
    } catch (error) {
      failAiGateway(error);
    }
    await runDetail(command, opts, (client) => client.telemetry[name](options));
  });
}

const groupDimensions = [...AI_GW_GROUP_DIMENSIONS, 'status_code', 'users'] as const;
type GroupDimension = (typeof groupDimensions)[number];
type GroupColumn = (typeof AI_GW_GROUP_COLUMNS)[number];

function isGroupDimension(value: string): value is GroupDimension {
  return groupDimensions.some((dimension) => dimension === value);
}

function isGroupColumn(value: string): value is GroupColumn {
  return AI_GW_GROUP_COLUMNS.some((column) => column === value);
}

/** CLI syntax only: retain the SDK's dimension/column lists and filter schema as authority. */
function groupOptionsFrom(
  dimension: GroupDimension,
  opts: WindowFlags & ChartFilterFlags,
): AIGatewayGroupOptions {
  const options: AIGatewayGroupOptions = { ...windowFrom(opts), ...chartFiltersFrom(opts) };
  if (opts.columns !== undefined) {
    if (dimension === 'users') throw new CliUsageError('User grouping does not support --columns');
    const columns = opts.columns.split(',').map((column) => column.trim());
    if (!columns.every(isGroupColumn)) {
      throw new CliUsageError(`Invalid --columns: choose from ${AI_GW_GROUP_COLUMNS.join(', ')}`);
    }
    options.columns = columns;
  }
  return options;
}

/** Register all SDK telemetry reads except the legacy cost renderer. */
export function registerAiGatewayTelemetryReads(telemetry: Command): void {
  const cache = showHelpOnEmpty(telemetry.command('cache').description('Inspect cache telemetry'));
  const cacheSummary = addWindowOptions(cache.command('summary').description('Get cache totals'));
  cacheSummary.action((opts: WindowFlags) =>
    runDetail(cacheSummary, opts, (client) => client.telemetry.cacheSummary(windowFrom(opts))),
  );
  const cacheTrend = addWindowOptions(cache.command('trend').description('Get cache-hit trend'));
  cacheTrend.action((opts: WindowFlags) =>
    runDetail(cacheTrend, opts, (client) => client.telemetry.cacheHitTrend(windowFrom(opts))),
  );

  registerMetric(telemetry, 'error-trends', 'Get error trends', 'errorTrends');
  registerMetric(telemetry, 'errors', 'Get error count', 'errors');

  const feedback = showHelpOnEmpty(
    telemetry.command('feedback').description('Inspect model feedback telemetry'),
  );
  const feedbackMethods = {
    distribution: 'feedbackScoreDistribution',
    models: 'feedbackModels',
    trend: 'feedbackTrend',
    weighted: 'feedbackWeighted',
  } as const;
  for (const [name, method] of Object.entries(feedbackMethods)) {
    const command = addWindowOptions(feedback.command(name).description(`Get feedback ${name}`));
    command.action((opts: WindowFlags) =>
      runDetail(command, opts, (client) => client.telemetry[method](windowFrom(opts))),
    );
  }

  const groupBy = addChartFilterOptions(
    addWindowOptions(
      telemetry
        .command('group-by <dimension>')
        .description(`Aggregate telemetry by ${groupDimensions.join(', ')}`)
        .option('--columns <names>', 'Comma-separated aggregate columns (not supported for users)'),
    ),
  );
  groupBy.action(async (dimension: string, opts: WindowFlags & ChartFilterFlags) => {
    let options: AIGatewayGroupOptions;
    try {
      if (!isGroupDimension(dimension)) {
        throw new CliUsageError(
          `Invalid grouping dimension: choose from ${groupDimensions.join(', ')}`,
        );
      }
      options = groupOptionsFrom(dimension, opts);
    } catch (error) {
      failAiGateway(error);
    }
    await runDetail(groupBy, opts, (client) => {
      if (dimension === 'users') return client.telemetry.byUser(options);
      if (dimension === 'status_code') return client.telemetry.byStatusCode(options);
      // Narrow without an unchecked cast after the pre-client validation above.
      if (isGroupDimension(dimension)) return client.telemetry.groupBy(dimension, options);
      throw new CliUsageError('Invalid grouping dimension');
    });
  });

  registerFilteredMetric(telemetry, 'latency', 'Get latency telemetry');

  const logs = showHelpOnEmpty(telemetry.command('logs').description('Inspect request logs'));
  const logsList = addWindowOptions(
    logs
      .command('list')
      .description('List request logs')
      .option('--page-size <n>', 'Rows per response', '50')
      .option('--status-code <code>', 'Filter by HTTP status')
      .option('--trace-id <id>', 'Return one trace id'),
  );
  logsList.action((opts: WindowFlags & { pageSize?: string }) =>
    runDetail(logsList, opts, (client) =>
      client.telemetry.logs({
        ...windowFrom(opts),
        pageSize: parsePositiveInteger(opts.pageSize ?? '50', '--page-size'),
        ...(opts.statusCode
          ? { statusCode: parsePositiveInteger(opts.statusCode, '--status-code') }
          : {}),
        ...(opts.traceId ? { traceId: opts.traceId } : {}),
      }),
    ),
  );

  registerFilteredMetric(telemetry, 'requests', 'Get request count');
  registerMetric(telemetry, 'rescued-retries', 'Get rescued retry telemetry', 'rescuedRetries');
  registerFilteredMetric(telemetry, 'tokens', 'Get token usage');
  registerMetric(telemetry, 'user-trends', 'Get user trends', 'userTrends');
  registerMetric(telemetry, 'users', 'Get unique-user count', 'users');
}
