import chalk from 'chalk';
import { type Command, Option } from 'commander';

/**
 * A flag renamed in the v2 standardization. The old spelling keeps working
 * (hidden from help) until v3; using it prints a one-line stderr notice.
 */
export interface DeprecatedAlias {
  /** Old flag as a commander option string, e.g. '--format <format>'. */
  oldFlag: string;
  /** camelCase opts key the old flag parses into, e.g. 'format'. */
  oldKey: string;
  /** Canonical flag name for the warning text, e.g. '--output'. */
  canonicalFlag: string;
  /** camelCase opts key the canonical flag parses into, e.g. 'output'. */
  canonicalKey: string;
}

const ALIASES = new WeakMap<Command, DeprecatedAlias[]>();

/** Register a hidden deprecated alias on a command. */
export function registerDeprecatedAlias(cmd: Command, alias: DeprecatedAlias): Command {
  cmd.addOption(new Option(alias.oldFlag).hideHelp());
  const list = ALIASES.get(cmd) ?? [];
  list.push(alias);
  ALIASES.set(cmd, list);
  return cmd;
}

/**
 * Map any used deprecated flags onto their canonical keys, warning on stderr.
 * The canonical flag wins when both are supplied. Call at the top of the
 * command action with the parsed opts; returns the same opts object.
 */
export function resolveDeprecatedAliases<T extends Record<string, unknown>>(
  cmd: Command,
  opts: T,
): T {
  for (const alias of ALIASES.get(cmd) ?? []) {
    const oldValue = opts[alias.oldKey];
    if (oldValue === undefined) continue;
    console.error(
      chalk.yellow(
        `  ⚠ ${alias.oldFlag.split(' ')[0]} is deprecated and will be removed in v3 — use ${alias.canonicalFlag}`,
      ),
    );
    const source = cmd.getOptionValueSource?.(alias.canonicalKey);
    if (opts[alias.canonicalKey] === undefined || source === 'default' || source === undefined) {
      (opts as Record<string, unknown>)[alias.canonicalKey] = oldValue;
    }
  }
  return opts;
}
