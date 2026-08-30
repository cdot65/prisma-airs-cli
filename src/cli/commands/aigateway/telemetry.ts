import type { AIGatewayWindowOptions } from '@cdot65/prisma-airs-sdk';
import type { Command } from 'commander';
import { CliUsageError } from '../../renderer/index.js';
import { addReadOutput, runDetail, showHelpOnEmpty } from './shared.js';
import { parseDateOption, parseIntegerOption } from './structured-input.js';

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

function parsePositiveInteger(value: unknown, flag: string): number {
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
  method:
    | 'errorTrends'
    | 'errors'
    | 'latency'
    | 'requests'
    | 'rescuedRetries'
    | 'tokens'
    | 'userTrends'
    | 'users',
): void {
  const command = addWindowOptions(telemetry.command(name).description(description));
  command.action((opts: WindowFlags) =>
    runDetail(command, opts, (client) => client.telemetry[method](windowFrom(opts))),
  );
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

  const groupBy = addWindowOptions(
    telemetry
      .command('group-by <dimension>')
      .description('Aggregate telemetry by provider, model, status, or another SDK dimension')
      .option('--columns <names>', 'Comma-separated aggregate columns'),
  );
  groupBy.action((dimension: string, opts: WindowFlags) =>
    runDetail(groupBy, opts, (client) =>
      client.telemetry.groupBy(dimension as never, {
        ...windowFrom(opts),
        ...(opts.columns
          ? { columns: opts.columns.split(',').map((value) => value.trim()) as never }
          : {}),
      }),
    ),
  );

  registerMetric(telemetry, 'latency', 'Get latency telemetry', 'latency');

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

  registerMetric(telemetry, 'requests', 'Get request count', 'requests');
  registerMetric(telemetry, 'rescued-retries', 'Get rescued retry telemetry', 'rescuedRetries');
  registerMetric(telemetry, 'tokens', 'Get token usage', 'tokens');
  registerMetric(telemetry, 'user-trends', 'Get user trends', 'userTrends');
  registerMetric(telemetry, 'users', 'Get unique-user count', 'users');
}
