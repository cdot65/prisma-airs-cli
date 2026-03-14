import chalk from 'chalk';

/** Render an error message to stderr. */
export function renderError(message: string): void {
  console.error(chalk.red(`\n  Error: ${message}\n`));
}
