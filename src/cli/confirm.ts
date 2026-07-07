import { ui, usageError } from './renderer/index.js';

/**
 * Interactive confirmation for destructive operations.
 *
 * Policy (see DESIGN.md streams/exit-code contract):
 * - `--force` bypasses the prompt entirely.
 * - Non-interactive (no TTY) without --force is a usage error (exit 2) — a
 *   script must state its intent explicitly.
 * - Interactive decline prints "Aborted" and exits 0 (a no-op, not a failure).
 */

/** Prompt implementation shape — matches @inquirer/prompts `confirm`. */
export type PromptFn = (opts: { message: string; default?: boolean }) => Promise<boolean>;

export interface ConfirmOptions {
  /** Verb phrase for the non-interactive refusal, e.g. 'delete profile "x"'. */
  action?: string;
  /** Injected prompt implementation (tests). Defaults to @inquirer/prompts confirm. */
  promptFn?: PromptFn;
  /** TTY override (tests). Defaults to process.stdout.isTTY. */
  isTTY?: boolean;
}

/**
 * Gate a destructive operation behind confirmation. Returns normally when the
 * operation may proceed; otherwise exits the process (2 for non-interactive
 * refusal, 0 for interactive decline).
 */
export async function confirmOrAbort(
  message: string,
  force: boolean,
  options: ConfirmOptions = {},
): Promise<void> {
  if (force) return;

  const interactive = options.isTTY ?? process.stdout.isTTY === true;
  if (!interactive) {
    usageError(
      `refusing to ${options.action ?? 'proceed'} without --force in non-interactive mode`,
    );
  }

  // Lazy-load inquirer so non-interactive commands never pay its startup cost.
  const prompt = options.promptFn ?? (await import('@inquirer/prompts')).confirm;
  const confirmed = await prompt({ message, default: false });
  if (!confirmed) {
    ui.info('Aborted');
    process.exit(0);
  }
}
