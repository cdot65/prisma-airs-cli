import type { BackupResult, RestoreResult } from '../../backup/types.js';
import { ui } from './ui.js';

/** Render header for backup/restore commands. */
export function renderBackupHeader(): void {
  ui.header('Prisma AIRS — Backup & Restore');
}

/** Render backup run summary. */
export function renderBackupSummary(results: BackupResult[], outputDir: string): void {
  const ok = results.filter((r) => r.status === 'ok');
  const failed = results.filter((r) => r.status === 'failed');

  if (ok.length > 0) {
    ui.section(`Backed up ${ok.length} target(s) to ${outputDir}:`);
    for (const r of ok) {
      ui.bullet(`${r.name} → ${r.filename}`, 'success');
    }
  }

  if (failed.length > 0) {
    ui.section(`Failed (${failed.length}):`);
    for (const r of failed) {
      ui.bullet(`${r.name}: ${r.error}`, 'error');
    }
  }

  console.log();
}

/** Render restore run summary. */
export function renderRestoreSummary(results: RestoreResult[]): void {
  const groups = { created: 0, updated: 0, skipped: 0, failed: 0 };
  for (const r of results) groups[r.action]++;

  ui.section('Restore results:');
  for (const r of results) {
    const kind = r.action === 'failed' ? 'error' : r.action === 'skipped' ? 'skip' : 'success';
    const suffix = r.error ? `: ${r.error}` : '';
    ui.bullet(`${r.name} — ${r.action}${suffix}`, kind);
  }

  const parts: string[] = [];
  if (groups.created) parts.push(`${groups.created} created`);
  if (groups.updated) parts.push(`${groups.updated} updated`);
  if (groups.skipped) parts.push(`${groups.skipped} skipped`);
  if (groups.failed) parts.push(`${groups.failed} failed`);
  console.log(`\n  Total: ${parts.join(', ')}\n`);
}
