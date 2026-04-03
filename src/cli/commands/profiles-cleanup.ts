import { execSync } from 'node:child_process';
import chalk from 'chalk';
import type { Command } from 'commander';
import type { SecurityProfileInfo } from '../../airs/types.js';
import {
  renderCleanupPreview,
  renderCleanupResult,
  renderError,
  renderRuntimeConfigHeader,
} from '../renderer/index.js';
import { createMgmtService } from './runtime.js';

export interface DuplicateGroup {
  name: string;
  keep: { id: string; revision: number };
  remove: Array<{ id: string; revision: number }>;
}

export interface CleanupDeleteResult {
  id: string;
  revision: number;
  name: string;
  status: 'ok' | 'failed';
  error?: string;
}

export function findDuplicateProfiles(
  profiles: Array<{ profileId: string; profileName: string; revision?: number }>,
): DuplicateGroup[] {
  const groups = new Map<string, Array<{ id: string; revision: number }>>();

  for (const p of profiles) {
    const rev = p.revision ?? 0;
    const entry = { id: p.profileId, revision: rev };
    const existing = groups.get(p.profileName);
    if (existing) {
      existing.push(entry);
    } else {
      groups.set(p.profileName, [entry]);
    }
  }

  const duplicates: DuplicateGroup[] = [];
  for (const [name, entries] of groups) {
    if (entries.length <= 1) continue;
    entries.sort((a, b) => b.revision - a.revision);
    duplicates.push({
      name,
      keep: entries[0],
      remove: entries.slice(1),
    });
  }

  return duplicates;
}

function resolveUpdatedBy(flag?: string): string {
  if (flag) return flag;
  try {
    return execSync('git config user.email', { encoding: 'utf-8' }).trim();
  } catch {
    throw new Error('--updated-by <email> is required (could not detect git user.email)');
  }
}

export function registerCleanupCommand(parent: Command): void {
  parent
    .command('cleanup')
    .description('Delete old profile revisions, keeping only the latest per name')
    .option('--force', 'Skip confirmation — proceed with deletion')
    .option('--updated-by <email>', 'Email for deletion audit (default: git user.email)')
    .option('--output <format>', 'Output format: pretty or json', 'pretty')
    .action(async (opts) => {
      try {
        const fmt = opts.output as 'pretty' | 'json';
        if (fmt === 'pretty') renderRuntimeConfigHeader();

        const service = await createMgmtService();

        // Paginate to fetch all profiles
        const allProfiles: SecurityProfileInfo[] = [];
        let offset = 0;
        const pageSize = 200;
        while (true) {
          const page = await service.listProfiles({ limit: pageSize, offset });
          allProfiles.push(...page.profiles);
          if (page.nextOffset == null) break;
          offset = page.nextOffset;
        }

        const groups = findDuplicateProfiles(allProfiles);

        if (groups.length === 0) {
          if (fmt === 'json') {
            console.log(JSON.stringify({ duplicates: [], summary: { deleted: 0, failed: 0 } }));
          } else {
            console.log(chalk.green('\n  No duplicate profiles found.\n'));
          }
          return;
        }

        renderCleanupPreview(groups, fmt);

        if (!opts.force) {
          if (fmt === 'pretty') {
            console.log(`  Pass ${chalk.bold('--force')} to delete these revisions.\n`);
          }
          return;
        }

        const updatedBy = resolveUpdatedBy(opts.updatedBy);
        const results: CleanupDeleteResult[] = [];

        for (const group of groups) {
          for (const entry of group.remove) {
            try {
              await service.forceDeleteProfile(entry.id, updatedBy);
              results.push({ ...entry, name: group.name, status: 'ok' });
              if (fmt === 'pretty') {
                console.log(`  ${chalk.green('✓')} ${group.name} rev ${entry.revision}`);
              }
            } catch (err) {
              const msg = err instanceof Error ? err.message : String(err);
              results.push({ ...entry, name: group.name, status: 'failed', error: msg });
              if (fmt === 'pretty') {
                console.log(`  ${chalk.red('✗')} ${group.name} rev ${entry.revision}: ${msg}`);
              }
            }
          }
        }

        renderCleanupResult(results, fmt);

        const failed = results.filter((r) => r.status === 'failed').length;
        if (failed > 0) process.exit(1);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });
}
