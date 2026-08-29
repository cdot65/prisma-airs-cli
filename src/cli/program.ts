import { readFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { Command } from 'commander';
import { registerAiGatewayCommand } from './commands/aigateway.js';
import { registerCompletionCommand } from './commands/completion.js';
import { registerConfigCommand } from './commands/config.js';
import { registerDoctorCommand } from './commands/doctor.js';
import { registerModelSecurityCommand } from './commands/modelsecurity.js';
import { registerRedteamCommand } from './commands/redteam.js';
import { registerRuntimeCommand } from './commands/runtime.js';
import { installDebugLogger } from './debug-logger.js';
import { fail, resolveOutput, setQuiet, ui } from './renderer/index.js';

const READ_COMMAND_NAMES = new Set([
  'categories',
  'consumption',
  'evaluation',
  'evaluations',
  'files',
  'get',
  'languages',
  'list',
  'pypi-auth',
  'query',
  'registry-credentials',
  'report',
  'stats',
  'status',
  'values',
  'version',
  'versions',
  'violation',
  'violations',
]);

/** Give every `list` subcommand an `ls` alias and every `delete` an `rm` alias. */
function applyListDeleteAliases(cmd: Command): void {
  for (const sub of cmd.commands) {
    if (sub.name() === 'list' && !sub.aliases().includes('ls')) sub.alias('ls');
    if (sub.name() === 'delete' && !sub.aliases().includes('rm')) sub.alias('rm');
    applyListDeleteAliases(sub);
  }
}

/** Fill the uniform read-command flag surface after command groups register themselves. */
function applyReadContractFlags(cmd: Command): void {
  for (const sub of cmd.commands) {
    const flags = () => sub.options.map((option) => option.long);
    if ((sub.name() === 'list' || sub.name() === 'get') && !flags().includes('--output')) {
      sub.option('--output <format>', 'Output format: pretty, table, markdown, csv, json, yaml');
    }
    if (sub.name() === 'list' && (flags().includes('--limit') || flags().includes('--offset'))) {
      if (!flags().includes('--limit')) sub.option('--limit <n>', 'Items per page', Number, 50);
      if (!flags().includes('--offset')) sub.option('--offset <n>', 'Item offset', Number, 0);
      if (!flags().includes('--all')) sub.option('--all', 'Walk all pages');
      if (!flags().includes('--max')) {
        sub.option('--max <n>', 'Maximum items with --all; 0 removes the cap', Number, 10_000);
      }
    }
    applyReadContractFlags(sub);
  }
}

/** Keep every help surface deterministic and easy to scan. */
function applySortedHelp(cmd: Command): void {
  cmd.configureHelp({ sortOptions: true, sortSubcommands: true });
  for (const sub of cmd.commands) applySortedHelp(sub);
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
    .option('--output <format>', 'Default output format for read commands')
    .option('--quiet', 'Suppress status and decorative output (data and errors still print)');

  program.hook('preAction', async (_thisCommand, actionCommand) => {
    const root = actionCommand.optsWithGlobals?.() ?? _thisCommand.opts();
    setQuiet(Boolean(root.quiet));
    if (
      READ_COMMAND_NAMES.has(actionCommand.name()) &&
      actionCommand.options.some((option) => option.long === '--output')
    ) {
      try {
        const format = await resolveOutput(actionCommand, actionCommand.opts());
        actionCommand.setOptionValueWithSource('output', format, 'implied');
      } catch (error) {
        fail(error);
      }
    }
    if (root.debug) {
      const logPath = join(homedir(), '.prisma-airs', `debug-api-${Date.now()}.jsonl`);
      installDebugLogger(logPath);
      ui.status(`Debug: API log → ${logPath}`);
    }
  });

  registerRuntimeCommand(program);
  registerRedteamCommand(program);
  registerModelSecurityCommand(program);
  registerAiGatewayCommand(program);
  registerConfigCommand(program);
  registerDoctorCommand(program);
  registerCompletionCommand(program);

  applyListDeleteAliases(program);
  applyReadContractFlags(program);
  applySortedHelp(program);

  return program;
}
