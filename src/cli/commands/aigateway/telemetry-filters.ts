import { type AIGatewayChartFilters, AIGatewayChartFiltersSchema } from '@cdot65/prisma-airs-sdk';
import type { Command } from 'commander';
import { CliUsageError } from '../../renderer/index.js';
import { parseIntegerOption, parseJsonOption } from './structured-input.js';

/** Command-line syntax only; semantic validation belongs to the SDK schema. */
export interface ChartFilterFlags {
  traceId?: string;
  metadata?: string;
  statusCodes?: string;
  apiKeyIds?: string;
  aiOrgModels?: string;
  totalUnitsMin?: string;
  totalUnitsMax?: string;
  costMin?: string;
  costMax?: string;
}

export function addChartFilterOptions(command: Command): Command {
  return command
    .option('--trace-id <id>', 'Filter by one trace ID')
    .option('--metadata <json>', 'Filter by string-valued metadata (JSON object)')
    .option('--status-codes <codes>', 'Comma-separated HTTP statuses (OR within the list)')
    .option('--api-key-ids <ids>', 'Comma-separated service key UUIDs (not secret key values)')
    .option(
      '--ai-org-models <pairs>',
      'Comma-separated provider__model pairs, e.g. openai__gpt-5.6-terra',
    )
    .option('--total-units-min <n>', 'Inclusive minimum total tokens (nonnegative integer)')
    .option('--total-units-max <n>', 'Inclusive maximum total tokens (nonnegative integer)')
    .option('--cost-min <cents>', 'Inclusive minimum cost in cents (fractional cents allowed)')
    .option('--cost-max <cents>', 'Inclusive maximum cost in cents (fractional cents allowed)');
}

function csv(value: string): string[] {
  const values = value.split(',').map((entry) => entry.trim());
  if (values.some((entry) => entry.length === 0)) throw new CliUsageError('Empty CSV element');
  return values;
}

function decimal(value: string): number {
  // Full JSON-number grammar: never accept parseFloat prefixes or Number's empty/hex coercion.
  if (!/^-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?$/.test(value)) {
    throw new CliUsageError('Expected a decimal number');
  }
  return Number(value);
}

/** Validate before constructing a client or resolving a workspace. Never echo input values. */
export function chartFiltersFrom(opts: ChartFilterFlags): AIGatewayChartFilters {
  const filters: Record<string, unknown> = {};
  try {
    if (opts.traceId !== undefined) filters.traceId = opts.traceId;
    if (opts.metadata !== undefined) filters.metadata = parseJsonOption(opts.metadata);
    if (opts.statusCodes !== undefined)
      filters.statusCodes = csv(opts.statusCodes).map(parseIntegerOption);
    if (opts.apiKeyIds !== undefined) filters.apiKeyIds = csv(opts.apiKeyIds);
    if (opts.aiOrgModels !== undefined) filters.aiOrgModels = csv(opts.aiOrgModels);
    for (const key of ['totalUnitsMin', 'totalUnitsMax'] as const) {
      if (opts[key] !== undefined) filters[key] = parseIntegerOption(opts[key]);
    }
    for (const key of ['costMin', 'costMax'] as const) {
      if (opts[key] !== undefined) filters[key] = decimal(opts[key]);
    }
  } catch {
    throw new CliUsageError('Invalid chart filters: check JSON, CSV and numeric flag syntax');
  }
  const result = AIGatewayChartFiltersSchema.safeParse(filters);
  if (!result.success) {
    throw new CliUsageError(
      'Invalid chart filters: check status codes, UUIDs, provider__model pairs, string metadata and inclusive nonnegative bounds',
    );
  }
  return result.data;
}
