import chalk from 'chalk';

/** Print a friendly diagnostic for an unhandled promise rejection and exit 1. */
export function handleUnhandledRejection(reason: unknown): void {
  const message = reason instanceof Error ? reason.message : String(reason);
  console.error(chalk.red(`\n  Unexpected error: ${message}`));
  console.error(chalk.dim('  Re-run with --debug to capture full API traffic.\n'));
  process.exit(1);
}

/** Register process-level guards. Call once from the CLI entry point. */
export function installProcessGuards(): void {
  process.on('unhandledRejection', handleUnhandledRejection);
}
