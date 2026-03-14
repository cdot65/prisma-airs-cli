#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import 'dotenv/config';
import { Command } from 'commander';
import { registerModelSecurityCommand } from './commands/modelsecurity.js';
import { registerRedteamCommand } from './commands/redteam.js';
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

program.parse();
