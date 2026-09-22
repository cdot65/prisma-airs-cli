import type { Command } from 'commander';
import { register as registerDictionaries } from './dictionaries.js';
import { register as registerFilteringProfiles } from './filtering-profiles.js';
import { register as registerGenerate } from './generate.js';
import { register as registerPatterns } from './patterns.js';
import { register as registerProfiles } from './profiles.js';
import { register as registerTransfer } from './transfer.js';

export function registerDlpCommands(runtime: Command): void {
  const dlp = runtime
    .command('dlp')
    .description(
      'DLP management (filtering-profiles, patterns, profiles, dictionaries, generate, backup/restore)',
    );
  registerFilteringProfiles(dlp);
  registerPatterns(dlp);
  registerProfiles(dlp);
  registerDictionaries(dlp);
  registerGenerate(dlp);
  registerTransfer(dlp);
}
