import type { Command } from 'commander';
import { loadConfig } from '../../config/loader.js';
import { JsonFileStore } from '../../persistence/store.js';
import { renderError, renderHeader, renderRunList } from '../renderer/index.js';

/** Register the `list` (or `runs`) command — lists all saved runs. */
export function registerListCommand(parent: Command, commandName = 'list'): void {
  parent
    .command(commandName)
    .description('List all saved generation runs')
    .action(async () => {
      try {
        renderHeader();
        const config = await loadConfig();
        const store = new JsonFileStore(config.dataDir);
        const runs = await store.list();
        renderRunList(runs);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });
}
