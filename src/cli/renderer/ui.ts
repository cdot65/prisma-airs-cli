import chalk from 'chalk';
import { formatOutput } from './common.js';

/**
 * CLI output primitives. See DESIGN.md for the full spec.
 *
 * Renderers compose these; they do not call chalk directly. Data goes to
 * stdout, everything else (status, errors) to stderr so that
 * `--output json | jq` always parses.
 */

const INDENT = '  ';

export type BulletKind = 'neutral' | 'success' | 'error' | 'warn' | 'skip' | 'flag';

const GLYPHS: Record<BulletKind | 'info', string> = {
  neutral: '•',
  success: '✓',
  error: '✗',
  warn: '⚠',
  skip: '○',
  flag: '●',
  info: 'ℹ',
};

const COLORS: Record<BulletKind | 'info', (s: string) => string> = {
  neutral: chalk.dim,
  success: chalk.green,
  error: chalk.red,
  warn: chalk.yellow,
  skip: chalk.yellow,
  flag: chalk.yellow,
  info: chalk.cyan,
};

export const ui = {
  /** Blank line, bold title, optional dim subtitle, blank line. */
  header(title: string, subtitle?: string): void {
    console.log(`\n${INDENT}${chalk.bold(title)}`);
    if (subtitle) console.log(`${INDENT}${chalk.dim(subtitle)}`);
    console.log('');
  },

  /** Bold section label. */
  section(label: string): void {
    console.log(`\n${INDENT}${chalk.bold(label)}\n`);
  },

  /** Aligned key/value block: dim keys padded to the longest key. */
  keyValue(pairs: Array<[string, unknown]>): void {
    const width = Math.max(...pairs.map(([k]) => k.length));
    for (const [k, v] of pairs) {
      console.log(`${INDENT}${chalk.dim(k.padEnd(width))}${INDENT}${String(v ?? '')}`);
    }
  },

  /** Canonical box-drawing table (see formatOutput in common.ts). */
  table(columns: { key: string; label: string }[], rows: Record<string, unknown>[]): void {
    const rendered = formatOutput(rows, columns, 'table');
    if (rendered) {
      console.log(
        rendered
          .split('\n')
          .map((l) => `${INDENT}${l}`)
          .join('\n'),
      );
    }
  },

  success(msg: string): void {
    console.log(`${INDENT}${COLORS.success(`${GLYPHS.success} ${msg}`)}`);
  },

  warn(msg: string): void {
    console.log(`${INDENT}${COLORS.warn(`${GLYPHS.warn} ${msg}`)}`);
  },

  info(msg: string): void {
    console.log(`${INDENT}${COLORS.info(`${GLYPHS.info} ${msg}`)}`);
  },

  /** Errors go to stderr. */
  error(msg: string): void {
    console.error(`${INDENT}${COLORS.error(`${GLYPHS.error} ${msg}`)}`);
  },

  /** List item with a semantic glyph. */
  bullet(msg: string, kind: BulletKind = 'neutral'): void {
    console.log(`${INDENT}${COLORS[kind](GLYPHS[kind])} ${msg}`);
  },

  /** Dim de-emphasized line. */
  dim(msg: string): void {
    console.log(`${INDENT}${chalk.dim(msg)}`);
  },

  /** Progress / status line — stderr so it never pollutes piped data. */
  status(msg: string): void {
    console.error(`${INDENT}${chalk.dim(msg)}`);
  },

  /** Standard empty-list phrase. */
  emptyList(resource: string): void {
    console.log(`${INDENT}${chalk.dim(`No ${resource} found`)}`);
  },
};
