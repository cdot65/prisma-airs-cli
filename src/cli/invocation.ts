/** The launcher may select one fixed display context; this never selects credentials. */
export function commandName(): 'airs-cli' | 'airs cli' {
  return process.env.AIRS_CLI_INVOKED_AS === 'airs cli' ? 'airs cli' : 'airs-cli';
}

/** Adapt authored human-readable command hints without rewriting data payloads. */
export function commandHint(text: string): string {
  return commandName() === 'airs cli' ? text.replaceAll('airs-cli ', 'airs cli ') : text;
}
