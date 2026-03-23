#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import 'dotenv/config';
import { Command } from 'commander';
import { registerModelSecurityCommand } from './commands/modelsecurity.js';
import { registerRedteamCommand } from './commands/redteam.js';
import { registerRuntimeCommand } from './commands/runtime.js';
import { installDebugLogger } from './debug-logger.js';

const __dirname = dirname(fileURLToPath(import.meta.url));
const pkg = JSON.parse(readFileSync(join(__dirname, '../../package.json'), 'utf-8'));

const program = new Command();

program
  .name('airs')
  .description(
    'CLI and library for Palo Alto Prisma AIRS — guardrail refinement, AI red teaming, model security scanning, profile audits',
  )
  .version(pkg.version)
  .option('--debug', 'Log all AIRS/SCM API requests and responses to a JSONL file');

// Install debug logger before subcommands run
program.hook('preAction', (_thisCommand, actionCommand) => {
  const root = actionCommand.optsWithGlobals?.() ?? _thisCommand.opts();
  if (root.debug) {
    const logPath = join(homedir(), '.prisma-airs', `debug-api-${Date.now()}.jsonl`);
    installDebugLogger(logPath);
    console.log(`  Debug: API log → ${logPath}\n`);
  }
});

// Primary command groups
registerRuntimeCommand(program);
registerRedteamCommand(program);
registerModelSecurityCommand(program);

program.parse();
