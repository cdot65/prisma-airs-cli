import chalk from 'chalk';
import type { BackupResult, RestoreResult } from '../../backup/types.js';

/** Render header for backup/restore commands. */
export function renderBackupHeader(): void {
  console.log(chalk.bold('\n  Prisma AIRS — Backup & Restore\n'));
}

/** Render backup run summary. */
export function renderBackupSummary(results: BackupResult[], outputDir: string): void {
  const ok = results.filter((r) => r.status === 'ok');
  const failed = results.filter((r) => r.status === 'failed');

  if (ok.length > 0) {
    console.log(chalk.bold(`\n  Backed up ${ok.length} target(s) to ${outputDir}:\n`));
    for (const r of ok) {
      console.log(`    ${chalk.green('✓')} ${r.name} → ${chalk.dim(r.filename)}`);
    }
  }

  if (failed.length > 0) {
    console.log(chalk.bold(`\n  Failed (${failed.length}):\n`));
    for (const r of failed) {
      console.log(`    ${chalk.red('✗')} ${r.name}: ${r.error}`);
    }
  }

  console.log();
}

/** Render restore run summary. */
export function renderRestoreSummary(results: RestoreResult[]): void {
  const groups = { created: 0, updated: 0, skipped: 0, failed: 0 };
  for (const r of results) groups[r.action]++;

  console.log(chalk.bold('\n  Restore results:\n'));
  for (const r of results) {
    const icon =
      r.action === 'failed'
        ? chalk.red('✗')
        : r.action === 'skipped'
          ? chalk.yellow('○')
          : chalk.green('✓');
    const suffix = r.error ? `: ${r.error}` : '';
    console.log(`    ${icon} ${r.name} — ${r.action}${suffix}`);
  }

  const parts: string[] = [];
  if (groups.created) parts.push(`${groups.created} created`);
  if (groups.updated) parts.push(`${groups.updated} updated`);
  if (groups.skipped) parts.push(`${groups.skipped} skipped`);
  if (groups.failed) parts.push(chalk.red(`${groups.failed} failed`));
  console.log(`\n  Total: ${parts.join(', ')}\n`);
}
