import type { Command } from 'commander';
import { registerDeprecatedAlias, resolveDeprecatedAliases } from './deprecated-flags.js';

/** Fallback page size used to convert --offset to a page when --limit is not given. */
export const DEFAULT_PAGE_SIZE = 50;

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
