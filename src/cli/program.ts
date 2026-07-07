import { readFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { Command } from 'commander';
import { registerConfigCommand } from './commands/config.js';
import { registerDoctorCommand } from './commands/doctor.js';
import { registerModelSecurityCommand } from './commands/modelsecurity.js';
import { registerRedteamCommand } from './commands/redteam.js';
import { registerRuntimeCommand } from './commands/runtime.js';
import { installDebugLogger } from './debug-logger.js';
import { ui } from './renderer/index.js';

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
    .option('--debug', 'Log all AIRS/SCM API requests and responses to a JSONL file');

  program.hook('preAction', (_thisCommand, actionCommand) => {
    const root = actionCommand.optsWithGlobals?.() ?? _thisCommand.opts();
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

  return program;
}
