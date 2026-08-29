import chalk from 'chalk';
import type { Command } from 'commander';
import { dump } from 'js-yaml';
import { loadConfig } from '../../config/loader.js';

export function renderError(message: string): void {
  console.error(chalk.red(`\n  Error: ${message}\n`));
}

export class CliUsageError extends Error {}

export function fail(err: unknown): never {
  if (err instanceof CliUsageError) usageError(err.message);
  const message = err instanceof Error ? err.message : String(err);
  const status =
    (err as { status?: number; statusCode?: number })?.status ??
    (err as { statusCode?: number })?.statusCode;
  console.error(chalk.red(`\n  ✗ Error: ${message}`));
  if (status !== undefined) {
    console.error(chalk.red(`    HTTP ${status}`));
    console.error(chalk.dim('    Re-run with --debug to capture full API traffic.'));
  }
  console.error('');
  process.exit(1);
}

export function usageError(message: string): never {
  console.error(chalk.red(`\n  ✗ ${message}\n`));
  process.exit(2);
}

export type OutputFormat = 'pretty' | 'table' | 'markdown' | 'csv' | 'json' | 'yaml';
export const OUTPUT_FORMATS: readonly OutputFormat[] = [
  'pretty',
  'table',
  'markdown',
  'csv',
  'json',
  'yaml',
];

export interface ResolveOutputOptions {
  allowed?: readonly OutputFormat[];
}

export async function resolveOutput(
  command: Command,
  opts: { output?: string },
  resolution: ResolveOutputOptions = {},
): Promise<OutputFormat> {
  const localIsExplicit = command.getOptionValueSource?.('output') === 'cli';
  const root = command.parent ? command.optsWithGlobals() : command.opts();
  let configured: string | undefined;
  try {
    configured = (await loadConfig()).defaultOutput;
  } catch (error) {
    if (process.env.PANW_CLI_OUTPUT !== undefined) configured = process.env.PANW_CLI_OUTPUT;
    else throw error;
  }
  const candidate = String(localIsExplicit ? opts.output : (root.output ?? configured ?? 'pretty'));
  if (!OUTPUT_FORMATS.includes(candidate as OutputFormat))
    throw new CliUsageError(
      `Invalid output format '${candidate}'. Expected: ${OUTPUT_FORMATS.join(', ')}`,
    );
  const format = candidate as OutputFormat;
  const allowed = resolution.allowed ?? OUTPUT_FORMATS;
  if (!allowed.includes(format))
    throw new CliUsageError(
      `Output format '${format}' is not supported here. Expected: ${allowed.join(', ')}`,
    );
  return format;
}

export interface OutputColumn {
  key: string;
  label: string;
}

function displayValue(value: unknown): string {
  if (value == null) return '';
  return typeof value === 'object' ? JSON.stringify(value) : String(value);
}

function csvCell(value: unknown): string {
  const text = displayValue(value);
  return /[",\r\n]/.test(text) ? `"${text.replace(/"/g, '""')}"` : text;
}

function markdownCell(value: unknown): string {
  return displayValue(value).replace(/\\/g, '\\\\').replace(/\|/g, '\\|').replace(/\r?\n/g, '<br>');
}

export function formatOutput(
  rows: Record<string, unknown>[],
  columns: OutputColumn[],
  format: OutputFormat,
): string {
  if (rows.length === 0) return format === 'json' ? '[]' : '';
  const projected = rows.map((row) => columns.map((column) => row[column.key]));
  switch (format) {
    case 'json':
      return JSON.stringify(rows, null, 2);
    case 'yaml':
      return dump(rows, { noRefs: true, lineWidth: -1 }).trimEnd();
    case 'csv':
      return [
        columns.map((column) => csvCell(column.label)).join(','),
        ...projected.map((values) => values.map(csvCell).join(',')),
      ].join('\n');
    case 'markdown': {
      const header = `| ${columns.map((column) => markdownCell(column.label)).join(' | ')} |`;
      const divider = `| ${columns.map(() => '---').join(' | ')} |`;
      return [
        header,
        divider,
        ...projected.map((values) => `| ${values.map(markdownCell).join(' | ')} |`),
      ].join('\n');
    }
    case 'table': {
      const values = projected.map((row) => row.map(displayValue));
      const widths = columns.map((column, index) =>
        Math.max(column.label.length, ...values.map((row) => row[index].length)),
      );
      const separator = widths.map((width) => '─'.repeat(width + 2)).join('┼');
      const header = columns
        .map((column, index) => ` ${column.label.padEnd(widths[index])} `)
        .join('│');
      const body = values.map((row) =>
        row.map((value, index) => ` ${value.padEnd(widths[index])} `).join('│'),
      );
      return [header, separator, ...body].join('\n');
    }
    default:
      return '';
  }
}
