/**
 * Format usage examples for `.addHelpText('after', ...)`.
 *
 * Style: blank line, 'Examples:' heading, each invocation indented with a
 * `$ ` prefix. Keep examples flag-accurate — canonical flags only.
 */
export function examples(...lines: string[]): string {
  return `\nExamples:\n${lines.map((l) => `  $ ${l}`).join('\n')}\n`;
}
