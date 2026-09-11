import type { Command } from 'commander';

/** Predefined (PANW-shipped) records are catalog content, not tenant
 * configuration: listings hide them by default and show them on request.
 */
export function predefinedFlag<T extends Command>(cmd: T): T {
  return cmd.option(
    '--include-predefined',
    'Include predefined (PANW-shipped) records; listings show only tenant-created records by default',
  );
}

/** Filter a listing to tenant-created records unless predefined were requested. */
export function visibleRecords<T extends { type?: string | null }>(
  records: T[],
  includePredefined: unknown,
): T[] {
  return includePredefined ? records : records.filter((record) => record.type !== 'predefined');
}
