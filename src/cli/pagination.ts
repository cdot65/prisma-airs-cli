import type { Command } from 'commander';
import { registerDeprecatedAlias, resolveDeprecatedAliases } from './deprecated-flags.js';
import { CliUsageError } from './renderer/common.js';

/** Fallback page size used to convert --offset to a page when --limit is not given. */
export const DEFAULT_PAGE_SIZE = 50;
export const DEFAULT_OFFSET_LIMIT = 100;
export const DEFAULT_MAX_ITEMS = 10_000;

export type PaginationDialect = 'offset' | 'skip' | 'page';

export interface ListParams {
  limit: number;
  offset: number;
  all: boolean;
  max: number;
}

/** Register the canonical pagination flags for a paginated list command. */
export function registerListFlags(
  command: Command,
  options: { dialect: PaginationDialect },
): Command {
  const defaultLimit = options.dialect === 'offset' ? DEFAULT_OFFSET_LIMIT : DEFAULT_PAGE_SIZE;
  return command
    .option('--limit <n>', `Items per page (default: ${defaultLimit})`, Number, defaultLimit)
    .option('--offset <n>', 'Item offset (default: 0)', Number, 0)
    .option('--all', 'Walk all pages')
    .option('--max <n>', 'Maximum items with --all; 0 removes the cap', Number, DEFAULT_MAX_ITEMS);
}

function integerFlag(name: string, value: unknown, minimum: number): number {
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed < minimum) {
    const qualifier = minimum === 1 ? 'a positive integer' : 'a non-negative integer';
    throw new CliUsageError(`${name} must be ${qualifier}`);
  }
  return parsed;
}

/** Parse and validate canonical list pagination flags. */
export function resolveListParams(
  command: Command,
  opts: Record<string, unknown>,
  options: { dialect: PaginationDialect },
): ListParams {
  const defaultLimit = options.dialect === 'offset' ? DEFAULT_OFFSET_LIMIT : DEFAULT_PAGE_SIZE;
  const limit = integerFlag('--limit', opts.limit ?? defaultLimit, 1);
  const offset = integerFlag('--offset', opts.offset ?? 0, 0);
  const max = integerFlag('--max', opts.max ?? DEFAULT_MAX_ITEMS, 0);
  const all = Boolean(opts.all);
  if (all && command.getOptionValueSource?.('offset') === 'cli') {
    throw new CliUsageError('--all cannot be combined with --offset');
  }
  return { limit, offset, all, max };
}

/**
 * Register hidden deprecated page-based aliases on a list command that now
 * exposes canonical `--limit`/`--offset`. The size-ish flag (`--size` or
 * `--page-size`) maps directly onto `--limit`; `--page` is kept verbatim and
 * converted by {@link resolvePageParams}.
 */
export function registerPageAliases(
  cmd: Command,
  opts: { sizeFlag: '--size' | '--page-size'; sizeKey: 'size' | 'pageSize' },
): void {
  registerDeprecatedAlias(cmd, {
    oldFlag: `${opts.sizeFlag} <n>`,
    oldKey: opts.sizeKey,
    canonicalFlag: '--limit',
    canonicalKey: 'limit',
  });
  registerDeprecatedAlias(cmd, {
    oldFlag: '--page <n>',
    oldKey: 'page',
    canonicalFlag: '--offset',
    canonicalKey: 'offset',
  });
}

/**
 * Resolve canonical `--limit`/`--offset` (plus deprecated `--page`/size
 * aliases) into the page-based params a page-only API expects.
 *
 * The offset rounds down to a page boundary: `page = floor(offset / limit)`
 * (plus `indexBase` for 1-indexed APIs). When the deprecated `--page` flag is
 * used, its value passes through unchanged (preserving legacy semantics); an
 * explicit `--offset` on the CLI wins over `--page`.
 */
export function resolvePageParams(
  cmd: Command,
  opts: Record<string, unknown>,
  options: { indexBase?: 0 | 1; fallbackSize?: number } = {},
): { page?: number; size?: number } {
  const { indexBase = 0, fallbackSize = DEFAULT_PAGE_SIZE } = options;
  resolveDeprecatedAliases(cmd, opts);

  const toInt = (v: unknown): number | undefined => {
    if (v === undefined || v === null || v === '') return undefined;
    const n = Number.parseInt(String(v), 10);
    return Number.isNaN(n) ? undefined : n;
  };

  const size = toInt(opts.limit);
  const legacyPage = toInt(opts.page);
  const offsetExplicit = cmd.getOptionValueSource?.('offset') === 'cli';

  let page: number | undefined;
  if (offsetExplicit) {
    const offset = toInt(opts.offset) ?? 0;
    page = Math.floor(offset / (size ?? fallbackSize)) + indexBase;
  } else if (legacyPage !== undefined) {
    page = legacyPage;
  } else {
    const offset = toInt(opts.offset);
    if (offset !== undefined) {
      page = Math.floor(offset / (size ?? fallbackSize)) + indexBase;
    }
  }

  return { page, size };
}
