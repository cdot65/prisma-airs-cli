import { randomUUID } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { Command } from 'commander';
import { registerAgentGuardCommand } from './commands/agentguard.js';
import { registerAiGatewayCommand } from './commands/aigateway.js';
import { registerCompletionCommand } from './commands/completion.js';
import { registerDoctorCommand } from './commands/doctor.js';
import { registerModelSecurityCommand } from './commands/modelsecurity.js';
import { registerRedteamCommand } from './commands/redteam.js';
import { registerRuntimeCommand } from './commands/runtime.js';
import { registerTenantCommand } from './commands/tenant.js';
import { installDebugLogger } from './debug-logger.js';
import { fail, resolveOutput, setQuiet, ui, usageError } from './renderer/index.js';

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
  'vulnerabilities',
]);

/** Give every `list` subcommand an `ls` alias and every `delete` an `rm` alias. */
function applyListDeleteAliases(cmd: Command): void {
  for (const sub of cmd.commands) {
    if (sub.name() === 'list' && !sub.aliases().includes('ls')) sub.alias('ls');
    const isHiddenCompatibilityCommand =
      sub.name() === 'delete' && Boolean((sub as Command & { _hidden?: boolean })._hidden);
    if (sub.name() === 'delete' && !isHiddenCompatibilityCommand && !sub.aliases().includes('rm'))
      sub.alias('rm');
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
  let dlpDebugBodyEnvironment: { previous: string | undefined } | undefined;
  const restoreDlpDebugBody = () => {
    if (!dlpDebugBodyEnvironment) return;
    const { previous } = dlpDebugBodyEnvironment;
    if (previous === undefined) delete process.env.PANW_AI_SEC_DEBUG_BODY;
    else process.env.PANW_AI_SEC_DEBUG_BODY = previous;
    dlpDebugBodyEnvironment = undefined;
  };
  program.hook('postAction', restoreDlpDebugBody);
  // Embedded callers can catch action failures instead of exiting the process.
  const parseAsync = program.parseAsync.bind(program);
  program.parseAsync = async (...args: Parameters<typeof program.parseAsync>) => {
    try {
      return await parseAsync(...args);
    } finally {
      restoreDlpDebugBody();
    }
  };
  program
    .name('airs')
    .description(
      'CLI and library for Palo Alto Prisma AIRS — guardrail refinement, AI red teaming, model security scanning, profile audits',
    )
    .version(pkg.version)
    .option('--debug', 'Write redacted API diagnostics to a private JSONL file')
    .option('--output <format>', 'Default output format for read commands')
    .option('--quiet', 'Suppress status and decorative output (data and errors still print)');

  program.hook('preAction', async (_thisCommand, actionCommand) => {
    const root = actionCommand.optsWithGlobals?.() ?? _thisCommand.opts();
    setQuiet(Boolean(root.quiet));
    let ancestor: Command | null = actionCommand;
    let agentGuard = false;
    let dlp = false;
    while (ancestor) {
      if (ancestor.name() === 'agentguard') agentGuard = true;
      if (ancestor.name() === 'dlp' && ancestor.parent?.name() === 'runtime') dlp = true;
      ancestor = ancestor.parent;
    }
    if (dlp) {
      // SDK 0.30.1 reads this per request. Never let SDK body logging bypass the CLI
      // logger's omission of DLP keywords, regexes, metadata, and reflected error payloads.
      dlpDebugBodyEnvironment = { previous: process.env.PANW_AI_SEC_DEBUG_BODY };
      process.env.PANW_AI_SEC_DEBUG_BODY = '0';
    }
    const profileTransfer =
      actionCommand.parent?.name() === 'profiles' &&
      actionCommand.parent.parent?.name() === 'runtime' &&
      ['backup', 'restore'].includes(actionCommand.name());
    if (
      profileTransfer &&
      (root.debug || /^(1|true|yes|on)$/i.test(process.env.PANW_AI_SEC_DEBUG?.trim() ?? ''))
    )
      usageError(
        'Disable --debug and PANW_AI_SEC_DEBUG for profile transfer; backups can contain sensitive policy configuration',
      );
    const isEnvironmentReport =
      actionCommand.name() === 'report' &&
      ['runtime', 'redteam', 'aigateway', 'agentguard'].includes(
        actionCommand.parent?.name() ?? '',
      ) &&
      !(actionCommand.parent?.name() === 'redteam' && actionCommand.args.length > 0);
    if (
      isEnvironmentReport &&
      (root.debug || /^(1|true|yes|on)$/i.test(process.env.PANW_AI_SEC_DEBUG?.trim() ?? ''))
    )
      usageError(
        'Disable --debug and PANW_AI_SEC_DEBUG for environment reports to avoid persisting sensitive traffic content',
      );
    if (
      !isEnvironmentReport &&
      actionCommand.parent?.name() !== 'tenant' &&
      !(actionCommand.name() === 'report' && actionCommand.parent?.name() === 'redteam') &&
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
      const logPath = join(
        process.cwd(),
        `debug-api-${Date.now()}-${randomUUID().slice(0, 8)}.jsonl`,
      );
      try {
        installDebugLogger(logPath, {
          omitBodies: agentGuard || dlp,
        });
      } catch {
        ui.error(
          'Cannot create the debug log in the current working directory. Run from a writable directory or omit --debug.',
        );
        process.exit(1);
      }
      ui.status(`Debug: API log → ${logPath}`);
    }
  });

  registerRuntimeCommand(program);
  registerRedteamCommand(program);
  registerModelSecurityCommand(program);
  registerAgentGuardCommand(program);
  registerAiGatewayCommand(program);
  registerTenantCommand(program);
  registerDoctorCommand(program);
  registerCompletionCommand(program);

  applyListDeleteAliases(program);
  applyReadContractFlags(program);
  applySortedHelp(program);

  return program;
}
