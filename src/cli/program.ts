import { readFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { Command } from 'commander';
import { registerCompletionCommand } from './commands/completion.js';
import { registerConfigCommand } from './commands/config.js';
import { registerDoctorCommand } from './commands/doctor.js';
import { registerModelSecurityCommand } from './commands/modelsecurity.js';
import { registerRedteamCommand } from './commands/redteam.js';
import { registerRuntimeCommand } from './commands/runtime.js';
import { installDebugLogger } from './debug-logger.js';
import { setQuiet, ui } from './renderer/index.js';

/** Give every `list` subcommand an `ls` alias and every `delete` an `rm` alias. */
function applyListDeleteAliases(cmd: Command): void {
  for (const sub of cmd.commands) {
    if (sub.name() === 'list' && !sub.aliases().includes('ls')) sub.alias('ls');
    if (sub.name() === 'delete' && !sub.aliases().includes('rm')) sub.alias('rm');
    applyListDeleteAliases(sub);
  }
}

export function buildProgram(): Command {
  const here = dirname(fileURLToPath(import.meta.url));
  const pkg = JSON.parse(readFileSync(join(here, '../../package.json'), 'utf-8'));

  const program = new Command();
  program
    .name('airs')
    .description(
      'CLI and library for Palo Alto Prisma AIRS — guardrail refinement, AI red teaming, model security scanning, profile audits',
    )
    .version(pkg.version)
    .option('--debug', 'Log all AIRS/SCM API requests and responses to a JSONL file')
    .option('--quiet', 'Suppress status and decorative output (data and errors still print)');

  program.hook('preAction', (_thisCommand, actionCommand) => {
    const root = actionCommand.optsWithGlobals?.() ?? _thisCommand.opts();
    setQuiet(Boolean(root.quiet));
    if (root.debug) {
      const logPath = join(homedir(), '.prisma-airs', `debug-api-${Date.now()}.jsonl`);
      installDebugLogger(logPath);
      ui.status(`Debug: API log → ${logPath}`);
    }
  });

  registerRuntimeCommand(program);
  registerRedteamCommand(program);
  registerModelSecurityCommand(program);
  registerConfigCommand(program);
  registerDoctorCommand(program);
  registerCompletionCommand(program);

  applyListDeleteAliases(program);

  return program;
}
