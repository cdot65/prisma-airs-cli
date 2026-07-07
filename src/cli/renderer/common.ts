import chalk from 'chalk';

/** Render an error message to stderr. */
export function renderError(message: string): void {
  console.error(chalk.red(`\n  Error: ${message}\n`));
}

/**
 * Standard failure path for command actions: print the error (with HTTP
 * status and a --debug hint when the error carries one) and exit 1.
 */
export function fail(err: unknown): never {
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

/** Validation / usage failure: print the message and exit 2. */
export function usageError(message: string): never {
  console.error(chalk.red(`\n  ✗ ${message}\n`));
  process.exit(2);
}

export type OutputFormat = 'pretty' | 'table' | 'csv' | 'json' | 'yaml';

export const OUTPUT_FORMATS: OutputFormat[] = ['pretty', 'table', 'csv', 'json', 'yaml'];

/**
 * Generic structured output for list commands.
 * `rows` is an array of flat key-value objects.
 * `columns` defines which keys to show and their header labels.
 */
export function formatOutput(
  rows: Record<string, unknown>[],
  columns: { key: string; label: string }[],
  format: OutputFormat,
): string {
  if (rows.length === 0) return '';

  const vals = (row: Record<string, unknown>, key: string) => String(row[key] ?? '');

  switch (format) {
    case 'json':
      return JSON.stringify(
        rows.map((r) => {
          const obj: Record<string, unknown> = {};
          for (const c of columns) obj[c.key] = r[c.key];
          return obj;
        }),
        null,
        2,
      );

    case 'csv': {
      const header = columns.map((c) => c.label).join(',');
      const lines = rows.map((r) =>
        columns
          .map((c) => {
            const v = vals(r, c.key);
            return v.includes(',') || v.includes('"') ? `"${v.replace(/"/g, '""')}"` : v;
          })
          .join(','),
      );
      return [header, ...lines].join('\n');
    }

    case 'yaml': {
      return rows
        .map((r) => columns.map((c) => `${c.key}: ${vals(r, c.key)}`).join('\n'))
        .join('\n---\n');
    }

    case 'table': {
      const widths = columns.map((c) =>
        Math.max(c.label.length, ...rows.map((r) => vals(r, c.key).length)),
      );
      const sep = widths.map((w) => '─'.repeat(w + 2)).join('┼');
      const header = columns.map((c, i) => ` ${c.label.padEnd(widths[i])} `).join('│');
      const body = rows.map((r) =>
        columns.map((c, i) => ` ${vals(r, c.key).padEnd(widths[i])} `).join('│'),
      );
      return [header, sep, ...body].join('\n');
    }

    default:
      return '';
  }
}
