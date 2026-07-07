import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname } from 'node:path';
import type { Command } from 'commander';
import { type ConfigEntry, inspectConfig, resolveConfigFilePath } from '../../config/loader.js';
import { ConfigSchema } from '../../config/schema.js';
import { examples } from '../examples.js';
import { fail, formatOutput, ui, usageError } from '../renderer/index.js';

export const CONFIG_KEYS: string[] = Object.keys(ConfigSchema.shape);

const SECRET_PATTERN = /key|secret|token|password/i;

export function isKnownKey(key: string): boolean {
  return CONFIG_KEYS.includes(key);
}

export function isSecretKey(key: string): boolean {
  return SECRET_PATTERN.test(key);
}

/** Mask a secret value: last 4 chars for long values, otherwise `***`. */
export function maskSecret(value: unknown): string {
  const str = value == null ? '' : String(value);
  return str.length >= 8 ? `***${str.slice(-4)}` : '***';
}

export interface ConfigRow extends Record<string, unknown> {
  key: string;
  value: string;
  source: string;
}

/** Flatten inspected config into display rows, masking secrets unless reveal. */
export function buildConfigRows(
  inspected: Record<string, ConfigEntry>,
  reveal: boolean,
): ConfigRow[] {
  return Object.entries(inspected).map(([key, entry]) => {
    const raw = entry.value == null ? '' : String(entry.value);
    const value = raw !== '' && isSecretKey(key) && !reveal ? maskSecret(raw) : raw;
    return { key, value, source: entry.source };
  });
}

type FileOpResult<T> = ({ ok: true } & T) | { ok: false; error: string };

/** Read the config file strictly: missing → {}, malformed → error. */
async function readConfigFileStrict(
  filePath: string,
): Promise<FileOpResult<{ data: Record<string, unknown> }>> {
  let raw: string;
  try {
    raw = await readFile(filePath, 'utf-8');
  } catch {
    return { ok: true, data: {} };
  }
  try {
    const parsed = JSON.parse(raw);
    if (parsed === null || typeof parsed !== 'object' || Array.isArray(parsed)) {
      return { ok: false, error: `Config file is not a JSON object: ${filePath}` };
    }
    return { ok: true, data: parsed as Record<string, unknown> };
  } catch {
    return { ok: false, error: `Config file is not valid JSON: ${filePath}` };
  }
}

/**
 * Set a key in the config file. Validates the resulting config through
 * ConfigSchema (value coerced, e.g. "7" → 7), preserves unknown keys,
 * creates the file and directory if missing.
 */
export async function setConfigValue(
  filePath: string,
  key: string,
  value: string,
): Promise<FileOpResult<{ value: unknown }>> {
  const read = await readConfigFileStrict(filePath);
  if (!read.ok) return read;

  const candidate = { ...read.data, [key]: value };
  const result = ConfigSchema.safeParse(candidate);
  if (!result.success) {
    const messages = result.error.issues.map((i) => `${i.path.join('.')}: ${i.message}`);
    return { ok: false, error: messages.join('; ') };
  }

  const coerced = (result.data as Record<string, unknown>)[key];
  const next = { ...read.data, [key]: coerced };
  await mkdir(dirname(filePath), { recursive: true });
  await writeFile(filePath, `${JSON.stringify(next, null, 2)}\n`, 'utf-8');
  return { ok: true, value: coerced };
}

/** Remove a key from the config file. Preserves everything else. */
export async function unsetConfigValue(
  filePath: string,
  key: string,
): Promise<FileOpResult<{ removed: boolean }>> {
  const read = await readConfigFileStrict(filePath);
  if (!read.ok) return read;

  if (!(key in read.data)) return { ok: true, removed: false };

  const next = { ...read.data };
  delete next[key];
  await writeFile(filePath, `${JSON.stringify(next, null, 2)}\n`, 'utf-8');
  return { ok: true, removed: true };
}

function assertKnownKey(key: string): void {
  if (!isKnownKey(key)) {
    usageError(`Unknown config key '${key}'. Valid keys: ${CONFIG_KEYS.join(', ')}`);
  }
}

const LIST_FORMATS = ['pretty', 'json', 'yaml'] as const;
type ListFormat = (typeof LIST_FORMATS)[number];

function parseListFormat(value: string): ListFormat {
  if (!(LIST_FORMATS as readonly string[]).includes(value)) {
    usageError(`Invalid --output '${value}'. Valid formats: ${LIST_FORMATS.join(', ')}`);
  }
  return value as ListFormat;
}

const COLUMNS = [
  { key: 'key', label: 'Key' },
  { key: 'value', label: 'Value' },
  { key: 'source', label: 'Source' },
];

export function registerConfigCommand(program: Command): void {
  const config = program
    .command('config')
    .description('Manage CLI configuration (~/.prisma-airs/config.json)')
    .addHelpText(
      'after',
      examples(
        'airs config list',
        'airs config set scanConcurrency 10',
        'airs config get mgmtTsgId',
      ),
    );

  config
    .command('list')
    .description('Show effective configuration with per-key source (env/file/default)')
    .option('--output <format>', 'Output format: pretty, json, or yaml', 'pretty')
    .option('--reveal', 'Show secret values in full')
    .action(async (opts) => {
      try {
        const fmt = parseListFormat(opts.output);
        const filePath = resolveConfigFilePath();
        const rows = buildConfigRows(await inspectConfig(), Boolean(opts.reveal));

        if (fmt === 'pretty') {
          ui.header('Configuration', filePath);
          ui.table(COLUMNS, rows);
          if (!opts.reveal) ui.dim('Secrets masked — pass --reveal to show full values.');
          console.log('');
        } else {
          console.log(formatOutput(rows, COLUMNS, fmt));
        }
      } catch (err) {
        fail(err);
      }
    });

  config
    .command('get <key>')
    .description('Print a single effective config value')
    .option('--reveal', 'Show the real value of a secret key')
    .action(async (key: string, opts) => {
      try {
        assertKnownKey(key);
        const inspected = await inspectConfig();
        const entry = inspected[key];
        const raw = entry.value == null ? '' : String(entry.value);

        if (raw !== '' && isSecretKey(key)) {
          if (opts.reveal) {
            ui.status(`Warning: printing secret value for '${key}'`);
            console.log(raw);
          } else {
            console.log(maskSecret(raw));
          }
        } else {
          console.log(raw);
        }
      } catch (err) {
        fail(err);
      }
    });

  config
    .command('set <key> <value>')
    .description('Set a config value in the config file (validated via schema)')
    .action(async (key: string, value: string) => {
      try {
        assertKnownKey(key);
        const filePath = resolveConfigFilePath();
        const result = await setConfigValue(filePath, key, value);
        if (!result.ok) usageError(result.error);
        const display = isSecretKey(key) ? maskSecret(result.value) : String(result.value);
        ui.success(`Set ${key} = ${display} in ${filePath}`);
      } catch (err) {
        fail(err);
      }
    });

  config
    .command('unset <key>')
    .description('Remove a key from the config file (defaults take over)')
    .action(async (key: string) => {
      try {
        assertKnownKey(key);
        const filePath = resolveConfigFilePath();
        const result = await unsetConfigValue(filePath, key);
        if (!result.ok) usageError(result.error);
        if (result.removed) {
          ui.success(`Removed ${key} from ${filePath}`);
        } else {
          ui.info(`${key} is not set in ${filePath} — nothing to do`);
        }
      } catch (err) {
        fail(err);
      }
    });

  config
    .command('path')
    .description('Print the config file path')
    .action(() => {
      console.log(resolveConfigFilePath());
    });
}
