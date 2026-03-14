#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import 'dotenv/config';
import chalk from 'chalk';
import { Command } from 'commander';
import { registerAuditCommand } from './commands/audit.js';
import { registerGenerateCommand } from './commands/generate.js';
import { registerListCommand } from './commands/list.js';
import { registerModelSecurityCommand } from './commands/modelsecurity.js';
import { registerRedteamCommand } from './commands/redteam.js';
import { registerReportCommand } from './commands/report.js';
import { registerResumeCommand } from './commands/resume.js';
import { registerRuntimeCommand } from './commands/runtime.js';

const __dirname = dirname(fileURLToPath(import.meta.url));
const pkg = JSON.parse(readFileSync(join(__dirname, '../../package.json'), 'utf-8'));

const program = new Command();

program
  .name('airs')
  .description(
    'CLI and library for Palo Alto Prisma AIRS — guardrail refinement, AI red teaming, model security scanning, profile audits',
  )
  .version(pkg.version);

// Primary command groups
registerRuntimeCommand(program);
registerRedteamCommand(program);
registerModelSecurityCommand(program);

// ---------------------------------------------------------------------------
// Backward-compatible top-level aliases (deprecated — use runtime subcommands)
// ---------------------------------------------------------------------------
const deprecationNotice = (oldCmd: string, newCmd: string) =>
  chalk.yellow(`[deprecated] "airs ${oldCmd}" → use "airs ${newCmd}"`);

function registerDeprecated(
  registerFn: (parent: Command, name?: string) => void,
  oldCmd: string,
  newCmd: string,
) {
  registerFn(program);
  const cmd = program.commands.find((c) => c.name() === oldCmd);
  if (cmd) {
    cmd.hook('preAction', () => {
      console.error(deprecationNotice(oldCmd, newCmd));
    });
  }
}

registerDeprecated(registerGenerateCommand, 'generate', 'runtime topics generate');
registerDeprecated(registerResumeCommand, 'resume', 'runtime topics resume');
registerDeprecated(registerReportCommand, 'report', 'runtime topics report');
registerDeprecated(registerListCommand, 'list', 'runtime topics runs');
registerDeprecated(registerAuditCommand, 'audit', 'runtime profiles audit');

program.parse();
