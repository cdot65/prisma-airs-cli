import * as fs from 'node:fs';
import * as path from 'node:path';
import type { Command } from 'commander';
import { SdkPromptSetService } from '../../airs/promptsets.js';
import { SdkRedTeamService } from '../../airs/redteam.js';
import type { RedTeamCategory } from '../../airs/types.js';
import { resolveOutputDir } from '../../backup/io.js';
import type { BackupFormat } from '../../backup/types.js';
import { redTeamClientOptions } from '../../config/client-options.js';
import { loadConfig } from '../../config/loader.js';
import { confirmOrAbort } from '../confirm.js';
import { registerDeprecatedAlias, resolveDeprecatedAliases } from '../deprecated-flags.js';
import { examples } from '../examples.js';
import {
  buildAttackListFootnote,
  fail,
  type OutputFormat,
  renderAdapterDetail,
  renderAdapterList,
  renderAdapterValidation,
  renderAttackList,
  renderAuthValidation,
  renderBackupHeader,
  renderBackupSummary,
  renderCategories,
  renderChannelDetail,
  renderChannelList,
  renderChannelStats,
  renderCustomAttackList,
  renderCustomReport,
  renderDynamicReport,
  renderErrorLogs,
  renderEulaContent,
  renderEulaStatus,
  renderInstanceDetail,
  renderInstanceResponse,
  renderLanguages,
  renderPromptDetail,
  renderPromptList,
  renderPromptSetDetail,
  renderPromptSetList,
  renderPropertyNames,
  renderPropertyValues,
  renderRedteamHeader,
  renderRegistryCredentials,
  renderRestoreSummary,
  renderScanList,
  renderScanProgress,
  renderScanStatus,
  renderStaticReport,
  renderTargetDetail,
  renderTargetList,
  renderTargetTemplates,
  renderVersionInfo,
  renderVersionInfoUnavailable,
  ui,
  usageError,
} from '../renderer/index.js';
import { backupTargets } from './backup.js';
import { restoreTargets } from './restore.js';

/** Create an SdkRedTeamService from config. */
async function createService() {
  const config = await loadConfig();
  return new SdkRedTeamService(redTeamClientOptions(config));
}

/** Create an SdkPromptSetService from config. */
async function createPromptSetService() {
  const config = await loadConfig();
  return new SdkPromptSetService(redTeamClientOptions(config));
}

/** Build the STATIC scan category payload from the available AIRS categories. */
export function buildDefaultCategories(categories: RedTeamCategory[]): Record<string, string[]> {
  return Object.fromEntries(
    categories.map((category) => [
      category.id,
      category.subCategories
        .map((subCategory) => subCategory.id)
        .filter((id) => id !== 'MULTI_TURN'),
    ]),
  );
}

/** Parse `--goals` arg as inline JSON array (starts with `[`) or path to a JSON file. */
export function parseAttackGoals(input: string): string[] {
  const trimmed = input.trim();
  const raw = trimmed.startsWith('[') ? trimmed : fs.readFileSync(trimmed, 'utf-8');
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch (err) {
    throw new Error(`--goals: invalid JSON (${err instanceof Error ? err.message : err})`);
  }
  if (!Array.isArray(parsed) || !parsed.every((g) => typeof g === 'string' && g.length > 0)) {
    throw new Error('--goals: expected a JSON array of non-empty strings');
  }
  return parsed;
}

/** Client-side slice for list commands whose API lacks pagination. */
export function sliceClientSide<T>(items: T[], opts: { limit?: string; offset?: string }): T[] {
  const offset = opts.offset !== undefined ? Number.parseInt(opts.offset, 10) : 0;
  const limit = opts.limit !== undefined ? Number.parseInt(opts.limit, 10) : undefined;
  return items.slice(offset, limit === undefined ? undefined : offset + limit);
}

/** Parse a string flag as a positive integer. */
export function parsePositiveInt(input: string, flag: string): number {
  const n = Number.parseInt(input, 10);
  if (!Number.isFinite(n) || n <= 0) {
    throw new Error(`${flag}: expected a positive integer, got "${input}"`);
  }
  return n;
}

/** Valid provider names for target init templates. */
export const VALID_TARGET_PROVIDERS = [
  'OPENAI',
  'HUGGING_FACE',
  'DATABRICKS',
  'BEDROCK',
  'REST',
  'STREAMING',
  'WEBSOCKET',
  'CUSTOM_TARGET_ADAPTER',
] as const;

/**
 * Providers whose connection goes through a REST HTTP endpoint.
 * These use RestConnectionParamsBase (api_endpoint / response_key)
 * rather than a native SDK provider config.
 */
const REST_PROVIDERS = new Set(['REST', 'STREAMING', 'WEBSOCKET', 'HUGGING_FACE']);

/** Build a target config scaffold from a provider template. */
export function buildTargetScaffold(
  provider: string,
  templates: Record<string, unknown>,
): Record<string, unknown> {
  const key = provider.toUpperCase();
  if (!VALID_TARGET_PROVIDERS.includes(key as (typeof VALID_TARGET_PROVIDERS)[number])) {
    throw new Error(
      `Unknown provider "${provider}". Valid providers: ${VALID_TARGET_PROVIDERS.join(', ')}`,
    );
  }

  // Custom target adapter — drives via an in-cluster Python sidecar.
  if (key === 'CUSTOM_TARGET_ADAPTER') {
    return {
      name: '',
      target_type: 'AGENT',
      connection_type: 'CUSTOM_TARGET_ADAPTER',
      api_endpoint_type: 'NETWORK_BROKER',
      network_broker_channel_uuid: '<channel-uuid>',
      adapter_uuid: '<adapter-uuid>',
      // adapter_variable_overrides is an ARRAY of {key, value, type} objects.
      adapter_variable_overrides: [],
      target_background: { use_case: '' },
      additional_context: {},
    };
  }

  // REST-family providers: use RestConnectionParamsBase schema.
  // The API expects api_endpoint / response_key, not the legacy url field
  // that getTargetTemplates() returns.
  if (REST_PROVIDERS.has(key)) {
    const tpl = (templates[key] ?? {}) as Record<string, unknown>;
    return {
      name: '',
      target_type: 'APPLICATION',
      connection_type: 'CUSTOM',
      api_endpoint_type: 'PUBLIC',
      response_mode: key === 'STREAMING' ? 'STREAMING' : key === 'WEBSOCKET' ? 'WEBSOCKET' : 'REST',
      auth_type: 'HEADERS',
      auth_config: {
        auth_header: { Authorization: 'Bearer <token>' },
      },
      connection_params: {
        api_endpoint: (tpl.url as string | undefined) ?? '',
        request_headers: { 'Content-Type': 'application/json' },
        request_json: tpl.request_json ?? { messages: [{ role: 'user', content: '{INPUT}' }] },
        response_json: tpl.response_json ?? { choices: [{ message: { content: '{RESPONSE}' } }] },
        response_key: 'choices.0.message.content',
      },
      target_background: {},
      additional_context: {},
    };
  }

  // Native SDK providers (OPENAI, BEDROCK, DATABRICKS): use NativeConnectionParamsBase.
  return {
    name: '',
    target_type: 'APPLICATION',
    connection_type: key,
    api_endpoint_type: 'PUBLIC',
    response_mode: 'REST',
    auth_type: 'HEADERS',
    auth_config: {
      auth_header: { Authorization: 'Bearer <token>' },
    },
    connection_params: {
      target_connection_config: templates[key] ?? {},
    },
    target_background: {},
    additional_context: {},
  };
}

/** Resolve --script-file / --script-b64 into a base64 script string. */
export function resolveScriptB64(opts: { scriptFile?: string; scriptB64?: string }): string {
  if (opts.scriptFile !== undefined && opts.scriptB64 !== undefined) {
    throw new Error('--script-file and --script-b64 are mutually exclusive');
  }
  if (opts.scriptB64 !== undefined) return opts.scriptB64;
  if (opts.scriptFile !== undefined) {
    return Buffer.from(fs.readFileSync(opts.scriptFile, 'utf-8')).toString('base64');
  }
  throw new Error('one of --script-file or --script-b64 is required');
}

/** Parse --variables as a JSON array of { key, value?, type: VAR|SECRET }. */
export function parseAdapterVariables(
  input: string,
): Array<{ key: string; value?: string | null; type: 'VAR' | 'SECRET' }> {
  let parsed: unknown;
  try {
    parsed = JSON.parse(input);
  } catch (err) {
    throw new Error(`--variables: invalid JSON (${err instanceof Error ? err.message : err})`);
  }
  if (
    !Array.isArray(parsed) ||
    !parsed.every(
      (v) =>
        v !== null &&
        typeof v === 'object' &&
        typeof (v as { key?: unknown }).key === 'string' &&
        ((v as { type?: unknown }).type === 'VAR' || (v as { type?: unknown }).type === 'SECRET'),
    )
  ) {
    throw new Error(
      '--variables: expected a JSON array of { "key": string, "value"?: string|null, "type": "VAR"|"SECRET" }',
    );
  }
  return parsed as Array<{ key: string; value?: string | null; type: 'VAR' | 'SECRET' }>;
}

/** Register the `redteam` command group. */
export function registerRedteamCommand(program: Command): void {
  const redteam = program.command('redteam').description('AI Red Team scan operations');

  // -----------------------------------------------------------------------
  // redteam abort — abort a running scan
  // -----------------------------------------------------------------------
  redteam
    .command('abort <jobId>')
    .description('Abort a running scan')
    .action(async (jobId: string) => {
      try {
        renderRedteamHeader();
        const service = await createService();
        await service.abortScan(jobId);
        ui.success(`Scan ${jobId} aborted.`);
      } catch (err) {
        fail(err);
      }
    });

  // -----------------------------------------------------------------------
  // redteam categories — list attack categories
  // -----------------------------------------------------------------------
  redteam
    .command('categories')
    .description('List available attack categories')
    .action(async () => {
      try {
        renderRedteamHeader();
        const service = await createService();
        const categories = await service.getCategories();
        renderCategories(categories);
      } catch (err) {
        fail(err);
      }
    });

  // -----------------------------------------------------------------------
  // redteam eula — EULA management subcommands
  // -----------------------------------------------------------------------
  const eula = redteam.command('eula').description('Manage Red Team EULA');

  eula
    .command('status')
    .description('Check EULA acceptance status')
    .action(async () => {
      try {
        renderRedteamHeader();
        const service = await createService();
        const status = await service.getEulaStatus();
        renderEulaStatus(status);
      } catch (err) {
        fail(err);
      }
    });

  eula
    .command('content')
    .description('Display EULA content')
    .action(async () => {
      try {
        renderRedteamHeader();
        const service = await createService();
        const content = await service.getEulaContent();
        renderEulaContent(content);
      } catch (err) {
        fail(err);
      }
    });

  const eulaAccept = eula
    .command('accept')
    .description('Accept the EULA')
    .option('--force', 'Skip confirmation prompt');
  registerDeprecatedAlias(eulaAccept, {
    oldFlag: '--confirm',
    oldKey: 'confirm',
    canonicalFlag: '--force',
    canonicalKey: 'force',
  });
  eulaAccept.action(async (opts) => {
    resolveDeprecatedAliases(eulaAccept, opts);
    try {
      renderRedteamHeader();
      const service = await createService();
      const content = await service.getEulaContent();

      if (!opts.force) {
        renderEulaContent(content);
        ui.dim('Pass --force to accept.');
        return;
      }

      const result = await service.acceptEula(content.content);
      renderEulaStatus(result);
      ui.success('EULA accepted.');
    } catch (err) {
      fail(err);
    }
  });

  // -----------------------------------------------------------------------
  // redteam instances — instance CRUD subcommands
  // -----------------------------------------------------------------------
  const instances = redteam.command('instances').description('Manage Red Team instances');

  instances
    .command('create')
    .description('Create an instance')
    .requiredOption('--tsg-id <id>', 'TSG ID')
    .requiredOption('--tenant-id <id>', 'Tenant ID')
    .requiredOption('--app-id <id>', 'App ID')
    .requiredOption('--region <region>', 'Region')
    .action(async (opts) => {
      try {
        renderRedteamHeader();
        const service = await createService();
        const result = await service.createInstance({
          tsgId: opts.tsgId,
          tenantId: opts.tenantId,
          appId: opts.appId,
          region: opts.region,
        });
        renderInstanceResponse(result);
      } catch (err) {
        fail(err);
      }
    });

  instances
    .command('get <tenantId>')
    .description('Get instance details')
    .option('--output <format>', 'Output format: pretty, json, yaml', 'pretty')
    .action(async (tenantId: string, opts) => {
      try {
        const fmt = opts.output as OutputFormat;
        if (fmt === 'pretty') renderRedteamHeader();
        const service = await createService();
        const result = await service.getInstance(tenantId);
        renderInstanceDetail(result, fmt);
      } catch (err) {
        fail(err);
      }
    });

  instances
    .command('update <tenantId>')
    .description('Update an instance')
    .requiredOption('--tsg-id <id>', 'TSG ID')
    .requiredOption('--app-id <id>', 'App ID')
    .requiredOption('--region <region>', 'Region')
    .action(async (tenantId: string, opts) => {
      try {
        renderRedteamHeader();
        const service = await createService();
        const result = await service.updateInstance(tenantId, {
          tsgId: opts.tsgId,
          tenantId,
          appId: opts.appId,
          region: opts.region,
        });
        renderInstanceResponse(result);
      } catch (err) {
        fail(err);
      }
    });

  instances
    .command('delete <tenantId>')
    .description('Delete an instance')
    .action(async (tenantId: string) => {
      try {
        renderRedteamHeader();
        const service = await createService();
        const result = await service.deleteInstance(tenantId);
        renderInstanceResponse(result);
        ui.success(`Instance ${tenantId} deleted.`);
      } catch (err) {
        fail(err);
      }
    });

  // -----------------------------------------------------------------------
  // redteam devices — device management subcommands
  // -----------------------------------------------------------------------
  const devices = redteam.command('devices').description('Manage Red Team devices');

  devices
    .command('create <tenantId>')
    .description('Create devices for an instance')
    .requiredOption('--config <path>', 'JSON file with device request')
    .action(async (tenantId: string, opts) => {
      try {
        renderRedteamHeader();
        const service = await createService();
        const config = JSON.parse(fs.readFileSync(opts.config, 'utf-8'));
        const result = await service.createDevices(tenantId, config);
        ui.success('Devices created:');
        console.log(JSON.stringify(result, null, 2));
      } catch (err) {
        fail(err);
      }
    });

  devices
    .command('update <tenantId>')
    .description('Update devices for an instance (PATCH)')
    .requiredOption('--config <path>', 'JSON file with device request')
    .action(async (tenantId: string, opts) => {
      try {
        renderRedteamHeader();
        const service = await createService();
        const config = JSON.parse(fs.readFileSync(opts.config, 'utf-8'));
        const result = await service.updateDevices(tenantId, config);
        ui.success('Devices updated:');
        console.log(JSON.stringify(result, null, 2));
      } catch (err) {
        fail(err);
      }
    });

  devices
    .command('delete <tenantId>')
    .description('Delete devices by serial numbers')
    .requiredOption('--serial-numbers <list>', 'Comma-separated serial numbers')
    .action(async (tenantId: string, opts) => {
      try {
        renderRedteamHeader();
        const service = await createService();
        const result = await service.deleteDevices(tenantId, opts.serialNumbers);
        ui.success('Devices deleted:');
        console.log(JSON.stringify(result, null, 2));
      } catch (err) {
        fail(err);
      }
    });

  // -----------------------------------------------------------------------
  // redteam registry-credentials — registry credentials
  // -----------------------------------------------------------------------
  redteam
    .command('registry-credentials')
    .description('Get or create registry credentials')
    .option('--output <format>', 'Output format: pretty, json, yaml', 'pretty')
    .action(async (opts) => {
      try {
        const fmt = opts.output as OutputFormat;
        if (fmt === 'pretty') renderRedteamHeader();
        const service = await createService();
        const creds = await service.getRegistryCredentials();
        renderRegistryCredentials(creds, fmt);
      } catch (err) {
        fail(err);
      }
    });

  // -----------------------------------------------------------------------
  // redteam list — list recent scans
  // -----------------------------------------------------------------------
  redteam
    .command('list')
    .description('List recent scans')
    .option('--status <status>', 'Filter by status (QUEUED, RUNNING, COMPLETED, FAILED, ABORTED)')
    .option('--type <type>', 'Filter by job type (STATIC, DYNAMIC, CUSTOM)')
    .option('--target <uuid>', 'Filter by target UUID')
    .option('--limit <n>', 'Max results', '10')
    .option('--output <format>', 'Output format: pretty, table, csv, json, yaml', 'pretty')
    .action(async (opts) => {
      try {
        const fmt = opts.output as OutputFormat;
        if (fmt === 'pretty') renderRedteamHeader();
        const service = await createService();
        const scans = await service.listScans({
          status: opts.status,
          jobType: opts.type,
          targetId: opts.target,
          limit: Number.parseInt(opts.limit, 10),
        });
        renderScanList(scans, fmt);
      } catch (err) {
        fail(err);
      }
    });

  // -----------------------------------------------------------------------
  // redteam prompt-sets — prompt set CRUD subcommands
  // -----------------------------------------------------------------------
  const promptSets = redteam.command('prompt-sets').description('Manage custom prompt sets');

  promptSets
    .command('list')
    .description('List custom prompt sets')
    .option('--limit <n>', 'Max results (client-side)')
    .option('--offset <n>', 'Starting offset (client-side)')
    .option('--output <format>', 'Output format: pretty, table, csv, json, yaml', 'pretty')
    .action(async (opts) => {
      try {
        const fmt = opts.output as OutputFormat;
        if (fmt === 'pretty') renderRedteamHeader();
        const service = await createPromptSetService();
        const sets = await service.listPromptSets();
        renderPromptSetList(sliceClientSide(sets, opts), fmt);
      } catch (err) {
        fail(err);
      }
    });

  promptSets
    .command('get <uuid>')
    .description('Get prompt set details')
    .option('--output <format>', 'Output format: pretty, json, yaml', 'pretty')
    .action(async (uuid: string, opts) => {
      try {
        const fmt = opts.output as OutputFormat;
        if (fmt === 'pretty') renderRedteamHeader();
        const service = await createPromptSetService();
        const { set: ps, versionInfo: info } = await service.getPromptSetWithVersionInfo(uuid);
        if (fmt === 'pretty') {
          renderPromptSetDetail(ps);
          if (info) renderVersionInfo(info);
          else renderVersionInfoUnavailable();
        } else {
          renderPromptSetDetail(ps, fmt, info);
        }
      } catch (err) {
        fail(err);
      }
    });

  promptSets
    .command('create')
    .description('Create a new prompt set')
    .requiredOption('--name <name>', 'Prompt set name')
    .option('--description <desc>', 'Prompt set description')
    .action(async (opts) => {
      try {
        renderRedteamHeader();
        const service = await createPromptSetService();
        const result = await service.createPromptSet(opts.name, opts.description);
        ui.success(`Prompt set created: ${result.uuid}`);
        ui.keyValue([['Name', result.name]]);
      } catch (err) {
        fail(err);
      }
    });

  promptSets
    .command('update <uuid>')
    .description('Update a prompt set')
    .option('--name <name>', 'New name')
    .option('--description <desc>', 'New description')
    .action(async (uuid: string, opts) => {
      try {
        renderRedteamHeader();
        const service = await createPromptSetService();
        const request: { name?: string; description?: string } = {};
        if (opts.name) request.name = opts.name;
        if (opts.description) request.description = opts.description;
        const result = await service.updatePromptSet(uuid, request);
        renderPromptSetDetail(result);
      } catch (err) {
        fail(err);
      }
    });

  promptSets
    .command('archive <uuid>')
    .description('Archive a prompt set')
    .option('--unarchive', 'Unarchive instead')
    .action(async (uuid: string, opts) => {
      try {
        renderRedteamHeader();
        const service = await createPromptSetService();
        const archive = !opts.unarchive;
        await service.archivePromptSet(uuid, archive);
        ui.success(`Prompt set ${uuid} ${archive ? 'archived' : 'unarchived'}.`);
      } catch (err) {
        fail(err);
      }
    });

  const promptSetsDownload = promptSets
    .command('download <uuid>')
    .description('Download CSV template for a prompt set')
    .option('--output-file <path>', 'Output file path');
  registerDeprecatedAlias(promptSetsDownload, {
    oldFlag: '--output <path>',
    oldKey: 'output',
    canonicalFlag: '--output-file',
    canonicalKey: 'outputFile',
  });
  promptSetsDownload.action(async (uuid: string, opts) => {
    resolveDeprecatedAliases(promptSetsDownload, opts);
    try {
      renderRedteamHeader();
      const service = await createPromptSetService();
      const csv = await service.downloadTemplate(uuid);
      const outPath = opts.outputFile || `${uuid}-template.csv`;
      fs.writeFileSync(outPath, csv, 'utf-8');
      ui.success(`Template saved to ${outPath}`);
    } catch (err) {
      fail(err);
    }
  });

  promptSets
    .command('upload <uuid> <file>')
    .description('Upload CSV prompts to a prompt set')
    .action(async (uuid: string, file: string) => {
      try {
        renderRedteamHeader();
        const service = await createPromptSetService();
        const content = fs.readFileSync(file);
        const filename = path.basename(file);
        const blob = new File([content], filename, { type: 'text/csv' });
        const result = await service.uploadPromptsCsv(uuid, blob);
        ui.success(result.message);
      } catch (err) {
        fail(err);
      }
    });

  // -----------------------------------------------------------------------
  // redteam prompts — individual prompt CRUD subcommands
  // -----------------------------------------------------------------------
  const prompts = redteam.command('prompts').description('Manage prompts within prompt sets');

  prompts
    .command('list <setUuid>')
    .description('List prompts in a prompt set')
    .option('--limit <n>', 'Max results', '50')
    .option('--output <format>', 'Output format: pretty, json, yaml', 'pretty')
    .action(async (setUuid: string, opts) => {
      try {
        const fmt = opts.output as OutputFormat;
        if (fmt === 'pretty') renderRedteamHeader();
        const service = await createPromptSetService();
        const list = await service.listPrompts(setUuid, {
          limit: Number.parseInt(opts.limit, 10),
        });
        renderPromptList(list, fmt);
      } catch (err) {
        fail(err);
      }
    });

  prompts
    .command('get <setUuid> <promptUuid>')
    .description('Get prompt details')
    .option('--output <format>', 'Output format: pretty, json, yaml', 'pretty')
    .action(async (setUuid: string, promptUuid: string, opts) => {
      try {
        const fmt = opts.output as OutputFormat;
        if (fmt === 'pretty') renderRedteamHeader();
        const service = await createPromptSetService();
        const prompt = await service.getPrompt(setUuid, promptUuid);
        renderPromptDetail(prompt, fmt);
      } catch (err) {
        fail(err);
      }
    });

  prompts
    .command('add <setUuid>')
    .description('Add a prompt to a prompt set')
    .requiredOption('--prompt <text>', 'Prompt text')
    .option('--goal <text>', 'Prompt goal')
    .action(async (setUuid: string, opts) => {
      try {
        renderRedteamHeader();
        const service = await createPromptSetService();
        const result = await service.addPrompt(setUuid, opts.prompt, opts.goal);
        ui.success(`Prompt added: ${result.uuid}`);
      } catch (err) {
        fail(err);
      }
    });

  prompts
    .command('update <setUuid> <promptUuid>')
    .description('Update a prompt')
    .option('--prompt <text>', 'New prompt text')
    .option('--goal <text>', 'New goal')
    .action(async (setUuid: string, promptUuid: string, opts) => {
      try {
        renderRedteamHeader();
        const service = await createPromptSetService();
        const request: { prompt?: string; goal?: string } = {};
        if (opts.prompt) request.prompt = opts.prompt;
        if (opts.goal) request.goal = opts.goal;
        const result = await service.updatePrompt(setUuid, promptUuid, request);
        renderPromptDetail(result);
      } catch (err) {
        fail(err);
      }
    });

  prompts
    .command('delete <setUuid> <promptUuid>')
    .description('Delete a prompt')
    .action(async (setUuid: string, promptUuid: string) => {
      try {
        renderRedteamHeader();
        const service = await createPromptSetService();
        await service.deletePrompt(setUuid, promptUuid);
        ui.success(`Prompt ${promptUuid} deleted.`);
      } catch (err) {
        fail(err);
      }
    });

  // -----------------------------------------------------------------------
  // redteam properties — property name/value management
  // -----------------------------------------------------------------------
  const properties = redteam.command('properties').description('Manage custom attack properties');

  properties
    .command('list')
    .description('List property names')
    .option('--limit <n>', 'Max results (client-side)')
    .option('--offset <n>', 'Starting offset (client-side)')
    .option('--output <format>', 'Output format: pretty, table, csv, json, yaml', 'pretty')
    .action(async (opts) => {
      try {
        const fmt = opts.output as OutputFormat;
        if (fmt === 'pretty') renderRedteamHeader();
        const service = await createPromptSetService();
        const names = await service.getPropertyNames();
        renderPropertyNames(sliceClientSide(names, opts), fmt);
      } catch (err) {
        fail(err);
      }
    });

  properties
    .command('create')
    .description('Create a property name')
    .requiredOption('--name <name>', 'Property name')
    .action(async (opts) => {
      try {
        renderRedteamHeader();
        const service = await createPromptSetService();
        const result = await service.createPropertyName(opts.name);
        ui.success(result.message);
      } catch (err) {
        fail(err);
      }
    });

  properties
    .command('values <name>')
    .description('List values for a property')
    .option('--output <format>', 'Output format: pretty, json, yaml', 'pretty')
    .action(async (name: string, opts) => {
      try {
        const fmt = opts.output as OutputFormat;
        if (fmt === 'pretty') renderRedteamHeader();
        const service = await createPromptSetService();
        const values = await service.getPropertyValues(name);
        renderPropertyValues(values, fmt);
      } catch (err) {
        fail(err);
      }
    });

  properties
    .command('add-value')
    .description('Create a property value')
    .requiredOption('--name <name>', 'Property name')
    .requiredOption('--value <value>', 'Property value')
    .action(async (opts) => {
      try {
        renderRedteamHeader();
        const service = await createPromptSetService();
        const result = await service.createPropertyValue(opts.name, opts.value);
        ui.success(result.message);
      } catch (err) {
        fail(err);
      }
    });

  // -----------------------------------------------------------------------
  // redteam report — view scan report
  // -----------------------------------------------------------------------
  redteam
    .command('report <jobId>')
    .description('View scan report')
    .option('--attacks', 'Include attack list', false)
    .option('--severity <level>', 'Filter attacks by severity')
    .option('--limit <n>', 'Max attacks to show', '20')
    .action(async (jobId: string, opts) => {
      try {
        renderRedteamHeader();
        const service = await createService();
        const job = await service.getScan(jobId);
        renderScanStatus(job);

        if (job.jobType === 'CUSTOM') {
          const report = await service.getCustomReport(jobId);
          renderCustomReport(report);
          if (opts.attacks) {
            const attacks = await service.listCustomAttacks(jobId, {
              limit: Number.parseInt(opts.limit, 10),
            });
            renderCustomAttackList(attacks);
          }
        } else if (job.jobType === 'DYNAMIC') {
          const report = await service.getDynamicReport(jobId);
          renderDynamicReport(report);
        } else {
          const report = await service.getStaticReport(jobId);
          renderStaticReport(report);
          if (opts.attacks) {
            const { attacks, totalItems } = await service.listAttacks(jobId, {
              severity: opts.severity,
              limit: Number.parseInt(opts.limit, 10),
            });
            const footnote = buildAttackListFootnote({
              severity: opts.severity,
              totalItems,
              severityBreakdown: report.severityBreakdown,
            });
            renderAttackList(attacks, { footnote });
          }
        }
      } catch (err) {
        fail(err);
      }
    });

  // -----------------------------------------------------------------------
  // redteam scan — execute a red team scan
  // -----------------------------------------------------------------------
  redteam
    .command('scan')
    .description('Execute a red team scan against a target')
    .requiredOption('--target <uuid>', 'Target UUID')
    .requiredOption('--name <name>', 'Scan name')
    .option('--type <type>', 'Job type: STATIC, DYNAMIC, or CUSTOM', 'STATIC')
    .option('--categories <json>', 'Category filter JSON (STATIC scans)')
    .option('--prompt-sets <uuids>', 'Comma-separated prompt set UUIDs (CUSTOM scans)')
    .option('--goals <file>', 'JSON file or inline JSON array of attack goals (DYNAMIC scans)')
    .option('--depth <number>', 'Max conversation turns per goal (DYNAMIC scans)', '10')
    .option('--breadth <number>', 'Parallel agents per goal (DYNAMIC scans)', '6')
    .option('--no-wait', 'Submit scan without waiting for completion')
    .addHelpText(
      'after',
      examples(
        'airs redteam scan --target <target-uuid> --name "nightly-static"',
        'airs redteam scan --target <target-uuid> --name "custom-run" --type CUSTOM --prompt-sets <set-uuid>',
        'airs redteam scan --target <target-uuid> --name "agent-probe" --type DYNAMIC --goals goals.json --no-wait',
      ),
    )
    .action(async (opts) => {
      let categories: Record<string, unknown> | undefined;
      let attackGoals: string[] | undefined;
      let streamDepth: number;
      let streamBreadth: number;
      try {
        if (opts.categories) {
          categories = JSON.parse(opts.categories);
        }
        attackGoals = opts.goals ? parseAttackGoals(opts.goals as string) : undefined;
        streamDepth = parsePositiveInt(opts.depth as string, '--depth');
        streamBreadth = parsePositiveInt(opts.breadth as string, '--breadth');
      } catch (err) {
        usageError(err instanceof Error ? err.message : String(err));
      }

      const customPromptSets = opts.promptSets
        ? (opts.promptSets as string).split(',').map((s: string) => s.trim())
        : undefined;

      try {
        renderRedteamHeader();
        const service = await createService();

        if (opts.type === 'STATIC' && !categories) {
          const defaultCategories = buildDefaultCategories(await service.getCategories());
          const categoryCount = Object.values(defaultCategories).reduce(
            (total, subCategories) => total + subCategories.length,
            0,
          );
          categories = defaultCategories;
          ui.status(
            `No --categories given — defaulting to all ${categoryCount} categories (MULTI_TURN excluded). Pass --categories to narrow the scan.`,
          );
        }

        ui.status(`Creating ${opts.type} scan "${opts.name}"...`);
        const job = await service.createScan({
          name: opts.name,
          targetUuid: opts.target,
          jobType: opts.type,
          categories,
          customPromptSets,
          attackGoals,
          streamDepth,
          streamBreadth,
        });

        renderScanStatus(job);

        if (opts.wait !== false) {
          ui.status('Waiting for completion...');
          const completed = await service.waitForCompletion(job.uuid, (progress) =>
            renderScanProgress(progress),
          );
          console.log('\n');
          renderScanStatus(completed);
          ui.keyValue([['Job ID', completed.uuid]]);
          ui.dim('Run `airs redteam report <jobId>` to view results.');
        } else {
          ui.keyValue([['Job ID', job.uuid]]);
          ui.dim('Run `airs redteam status <jobId>` to check progress.');
        }
      } catch (err) {
        fail(err);
      }
    });

  // -----------------------------------------------------------------------
  // redteam status — check scan status
  // -----------------------------------------------------------------------
  redteam
    .command('status <jobId>')
    .description('Check scan status')
    .action(async (jobId: string) => {
      try {
        renderRedteamHeader();
        const service = await createService();
        const job = await service.getScan(jobId);
        renderScanStatus(job);
      } catch (err) {
        fail(err);
      }
    });

  // -----------------------------------------------------------------------
  // redteam targets — target CRUD subcommands
  // -----------------------------------------------------------------------
  const targets = redteam.command('targets').description('Manage red team targets');

  targets
    .command('list')
    .description('List configured red team targets')
    .option('--limit <n>', 'Max results (client-side)')
    .option('--offset <n>', 'Starting offset (client-side)')
    .option('--output <format>', 'Output format: pretty, table, csv, json, yaml', 'pretty')
    .addHelpText(
      'after',
      examples(
        'airs redteam targets list',
        'airs redteam targets list --output json',
        'airs redteam targets ls --limit 5',
      ),
    )
    .action(async (opts) => {
      try {
        const fmt = opts.output as OutputFormat;
        if (fmt === 'pretty') renderRedteamHeader();
        const service = await createService();
        const list = await service.listTargets();
        renderTargetList(sliceClientSide(list, opts), fmt);
      } catch (err) {
        fail(err);
      }
    });

  targets
    .command('get <uuid>')
    .description('Get target details')
    .option('--output <format>', 'Output format: pretty, json, yaml', 'pretty')
    .action(async (uuid: string, opts) => {
      try {
        const fmt = opts.output as OutputFormat;
        if (fmt === 'pretty') renderRedteamHeader();
        const service = await createService();
        const target = await service.getTarget(uuid);
        renderTargetDetail(target, fmt);
      } catch (err) {
        fail(err);
      }
    });

  targets
    .command('create')
    .description('Create a new red team target')
    .requiredOption('--config <path>', 'JSON file with target configuration')
    .option('--validate', 'Validate target connection before saving')
    .action(async (opts) => {
      try {
        renderRedteamHeader();
        const service = await createService();
        const config = JSON.parse(fs.readFileSync(opts.config, 'utf-8'));
        const target = await service.createTarget(
          config,
          opts.validate ? { validate: true } : undefined,
        );
        ui.success(`Target created: ${target.uuid}`);
        renderTargetDetail(target);
      } catch (err) {
        fail(err);
      }
    });

  targets
    .command('update <uuid>')
    .description('Update a red team target')
    .requiredOption('--config <path>', 'JSON file with target updates')
    .option('--validate', 'Validate target connection before saving')
    .action(async (uuid: string, opts) => {
      try {
        renderRedteamHeader();
        const service = await createService();
        const config = JSON.parse(fs.readFileSync(opts.config, 'utf-8'));
        const target = await service.updateTarget(
          uuid,
          config,
          opts.validate ? { validate: true } : undefined,
        );
        ui.success(`Target updated: ${target.uuid}`);
        renderTargetDetail(target);
      } catch (err) {
        fail(err);
      }
    });

  targets
    .command('delete <uuid>')
    .description('Delete a red team target')
    .option('--force', 'Skip confirmation prompt')
    .action(async (uuid: string, opts) => {
      try {
        await confirmOrAbort(`Delete red team target ${uuid}?`, Boolean(opts.force), {
          action: `delete target ${uuid}`,
        });
        renderRedteamHeader();
        const service = await createService();
        await service.deleteTarget(uuid);
        ui.success(`Target ${uuid} deleted.`);
      } catch (err) {
        fail(err);
      }
    });

  targets
    .command('probe')
    .description('Test target connection without saving')
    .requiredOption('--config <path>', 'JSON file with connection params')
    .action(async (opts) => {
      try {
        renderRedteamHeader();
        const service = await createService();
        const config = JSON.parse(fs.readFileSync(opts.config, 'utf-8'));
        const result = await service.probeTarget(config);
        ui.dim('Probe result:');
        console.log(JSON.stringify(result, null, 2));
      } catch (err) {
        fail(err);
      }
    });

  targets
    .command('profile <uuid>')
    .description('View target profile')
    .action(async (uuid: string) => {
      try {
        renderRedteamHeader();
        const service = await createService();
        const profile = await service.getTargetProfile(uuid);
        ui.dim('Target Profile:');
        console.log(JSON.stringify(profile, null, 2));
      } catch (err) {
        fail(err);
      }
    });

  targets
    .command('update-profile <uuid>')
    .description('Update target profile')
    .requiredOption('--config <path>', 'JSON file with profile updates')
    .action(async (uuid: string, opts) => {
      try {
        renderRedteamHeader();
        const service = await createService();
        const config = JSON.parse(fs.readFileSync(opts.config, 'utf-8'));
        const result = await service.updateTargetProfile(uuid, config);
        ui.success('Profile updated:');
        console.log(JSON.stringify(result, null, 2));
      } catch (err) {
        fail(err);
      }
    });

  targets
    .command('validate-auth')
    .description('Validate target auth credentials')
    .requiredOption('--auth-type <type>', 'Auth type: HEADERS, BASIC_AUTH, OAUTH2')
    .requiredOption('--config <path>', 'JSON file with auth_config')
    .option('--target-id <uuid>', 'Existing target UUID')
    .action(async (opts) => {
      try {
        renderRedteamHeader();
        const service = await createService();
        const authConfig = JSON.parse(fs.readFileSync(opts.config, 'utf-8'));
        const result = await service.validateTargetAuth({
          authType: opts.authType,
          authConfig,
          targetId: opts.targetId,
        });
        renderAuthValidation(result);
      } catch (err) {
        fail(err);
      }
    });

  targets
    .command('metadata')
    .description('Get target field metadata')
    .action(async () => {
      try {
        const service = await createService();
        const metadata = await service.getTargetMetadata();
        console.log(JSON.stringify(metadata, null, 2));
      } catch (err) {
        fail(err);
      }
    });

  const targetsInit = targets
    .command('init <provider>')
    .description('Scaffold a target config JSON from a provider template')
    .option('--output-file <file>', 'Output file path');
  registerDeprecatedAlias(targetsInit, {
    oldFlag: '--output <file>',
    oldKey: 'output',
    canonicalFlag: '--output-file',
    canonicalKey: 'outputFile',
  });
  targetsInit.action(async (provider: string, opts) => {
    resolveDeprecatedAliases(targetsInit, opts);
    if (
      !VALID_TARGET_PROVIDERS.includes(
        provider.toUpperCase() as (typeof VALID_TARGET_PROVIDERS)[number],
      )
    ) {
      usageError(
        `Unknown provider "${provider}". Valid providers: ${VALID_TARGET_PROVIDERS.join(', ')}`,
      );
    }
    const filename = opts.outputFile ?? `${provider.toLowerCase()}-target.json`;
    const outputPath = path.resolve(filename);
    if (fs.existsSync(outputPath)) {
      usageError(
        `File already exists: ${outputPath} (use --output-file to specify a different path)`,
      );
    }
    try {
      renderRedteamHeader();
      const service = await createService();
      const templates = await service.getTargetTemplates();
      const scaffold = buildTargetScaffold(provider, templates);
      fs.writeFileSync(outputPath, `${JSON.stringify(scaffold, null, 2)}\n`);
      ui.success('Target config scaffolded');
      ui.keyValue([
        ['File', outputPath],
        ['Provider', provider.toUpperCase()],
      ]);
      ui.dim('Next steps: edit the file to fill in name and credentials, then run:');
      ui.dim(`  airs redteam targets create --config ${filename} --validate`);
    } catch (err) {
      fail(err);
    }
  });

  targets
    .command('templates')
    .description('Get provider-specific target templates')
    .action(async () => {
      try {
        renderRedteamHeader();
        const service = await createService();
        const templates = await service.getTargetTemplates();
        renderTargetTemplates(templates);
      } catch (err) {
        fail(err);
      }
    });

  const targetsBackup = targets
    .command('backup')
    .description('Backup red team targets to local JSON/YAML files')
    .option('--output-dir <path>', 'Output directory')
    .option('--output <format>', 'Output format: json or yaml', 'json')
    .option('--name <targetName>', 'Backup a single target by name');
  registerDeprecatedAlias(targetsBackup, {
    oldFlag: '--format <format>',
    oldKey: 'format',
    canonicalFlag: '--output',
    canonicalKey: 'output',
  });
  targetsBackup.action(async (opts) => {
    resolveDeprecatedAliases(targetsBackup, opts);
    try {
      renderBackupHeader();
      const outputDir = resolveOutputDir(opts.outputDir, 'targets');
      const results = await backupTargets({
        outputDir,
        format: (opts.output ?? 'json') as BackupFormat,
        name: opts.name,
      });
      renderBackupSummary(results, outputDir);
      const failed = results.filter((r) => r.status === 'failed').length;
      if (failed > 0) process.exit(1);
    } catch (err) {
      fail(err);
    }
  });

  targets
    .command('restore')
    .description('Restore red team targets from local JSON/YAML backup files')
    .option('--input-dir <path>', 'Directory containing backup files')
    .option('--file <path>', 'Single backup file to restore')
    .option('--overwrite', 'Update existing targets with same name (default: skip)')
    .option('--validate', 'Validate target connection before saving')
    .action(async (opts) => {
      if (!opts.file && !opts.inputDir) {
        usageError('Specify --file <path> or --input-dir <path>');
      }
      try {
        renderBackupHeader();
        const results = await restoreTargets({
          file: opts.file,
          inputDir: opts.inputDir,
          overwrite: opts.overwrite,
          validate: opts.validate,
        });
        renderRestoreSummary(results);
        const failed = results.filter((r) => r.action === 'failed').length;
        if (failed > 0) process.exit(1);
      } catch (err) {
        fail(err);
      }
    });

  targets
    .command('error-logs <targetId>')
    .description('List target-profile error logs')
    .option('--limit <n>', 'Max results')
    .option('--offset <n>', 'Starting offset')
    .option('--search <text>', 'Filter by search text')
    .option('--output <format>', 'Output format: pretty, table, csv, json, yaml', 'pretty')
    .addHelpText('after', examples('airs redteam targets error-logs <targetId>'))
    .action(async (targetId: string, opts) => {
      try {
        const fmt = opts.output as OutputFormat;
        if (fmt === 'pretty') renderRedteamHeader();
        const service = await createService();
        const { logs } = await service.getTargetProfileErrorLogs(targetId, {
          limit: opts.limit ? parsePositiveInt(opts.limit, '--limit') : undefined,
          offset: opts.offset ? Number.parseInt(opts.offset, 10) : undefined,
          search: opts.search,
        });
        renderErrorLogs(logs, fmt);
      } catch (err) {
        fail(err);
      }
    });

  // -------------------------------------------------------------------------
  // Network Broker
  // -------------------------------------------------------------------------
  // -------------------------------------------------------------------------
  // redteam adapter — custom target adapters (SDK 0.16.0)
  // -------------------------------------------------------------------------
  const adapter = redteam
    .command('adapter')
    .description('Manage custom target adapters (scripted targets run via the network broker)');

  /** Fail early with a clear message when the broker channel is not ONLINE. */
  async function assertChannelOnline(
    service: Awaited<ReturnType<typeof createService>>,
    channelUuid: string,
  ): Promise<void> {
    let status: string | null | undefined;
    try {
      status = (await service.getChannel(channelUuid)).status;
    } catch {
      return; // channel lookup failing shouldn't mask the real operation's error
    }
    if (status && status !== 'ONLINE') {
      fail(
        new Error(
          `network broker channel ${channelUuid} is ${status} — adapter validation requires an ONLINE channel (network broker v1.4.0+). Check 'airs redteam network-broker channels list'.`,
        ),
      );
    }
  }

  adapter
    .command('list')
    .description('List custom target adapters')
    .option('--limit <n>', 'Max results')
    .option('--offset <n>', 'Starting offset')
    .option('--search <text>', 'Filter by search text')
    .option('--output <format>', 'Output format: pretty, table, csv, json, yaml', 'pretty')
    .action(async (opts) => {
      try {
        const fmt = opts.output as OutputFormat;
        if (fmt === 'pretty') renderRedteamHeader();
        const service = await createService();
        const { adapters, totalItems } = await service.listAdapters({
          limit: opts.limit ? parsePositiveInt(opts.limit, '--limit') : undefined,
          offset: opts.offset ? Number.parseInt(opts.offset, 10) : undefined,
          search: opts.search,
        });
        renderAdapterList(adapters, fmt, totalItems);
      } catch (err) {
        fail(err);
      }
    });

  adapter
    .command('get <uuid>')
    .description('Get a custom target adapter')
    .option('--output <format>', 'Output format: pretty, json, yaml', 'pretty')
    .action(async (uuid: string, opts) => {
      try {
        const fmt = opts.output as OutputFormat;
        if (fmt === 'pretty') renderRedteamHeader();
        const service = await createService();
        renderAdapterDetail(await service.getAdapter(uuid), fmt);
      } catch (err) {
        fail(err);
      }
    });

  adapter
    .command('create')
    .description('Create a custom target adapter')
    .requiredOption('--name <name>', 'Adapter name')
    .requiredOption(
      '--prompt <text>',
      'Sample prompt used to exercise the adapter during validation (not stored)',
    )
    .option('--script-file <path>', 'Path to the adapter script (encoded to base64 for you)')
    .option('--script-b64 <b64>', 'Adapter script, already base64-encoded')
    .option('--description <text>', 'Adapter description')
    .option('--channel <uuid>', 'Network broker channel UUID (required to activate)')
    .option('--variables <json>', 'JSON array of { key, value, type: VAR|SECRET }')
    .option('--draft', 'Save as DRAFT without running the validation script')
    .option('--output <format>', 'Output format: pretty, json, yaml', 'pretty')
    .addHelpText(
      'after',
      examples(
        `airs redteam adapter create --name my-adapter --script-file ./adapter.py --channel 550e8400-... --prompt 'Hello' --variables '[{"key":"endpoint","value":"http://agent.svc:8080","type":"VAR"}]'`,
        'airs redteam adapter create --name my-adapter --script-file ./adapter.py --prompt Hello --draft',
      ),
    )
    .action(async (opts) => {
      try {
        const fmt = opts.output as OutputFormat;
        if (fmt === 'pretty') renderRedteamHeader();
        const scriptB64 = resolveScriptB64(opts);
        const variables = opts.variables ? parseAdapterVariables(opts.variables) : undefined;
        const service = await createService();
        if (!opts.draft && opts.channel) await assertChannelOnline(service, opts.channel);
        const created = await service.createAdapter(
          {
            name: opts.name,
            scriptB64,
            prompt: opts.prompt,
            description: opts.description,
            networkBrokerChannelUuid: opts.channel,
            variables,
          },
          opts.draft ? false : undefined,
        );
        ui.success(`Adapter created: ${created.uuid}`);
        renderAdapterDetail(created, fmt);
      } catch (err) {
        fail(err);
      }
    });

  adapter
    .command('update <uuid>')
    .description(
      'Update a custom target adapter (read-modify-write; variables preserved unless --variables)',
    )
    .requiredOption(
      '--prompt <text>',
      'Sample validation prompt — required on every update because upstream never stores it',
    )
    .option('--name <name>', 'New adapter name')
    .option('--script-file <path>', 'New adapter script file (encoded to base64 for you)')
    .option('--script-b64 <b64>', 'New adapter script, already base64-encoded')
    .option('--description <text>', 'New description')
    .option('--channel <uuid>', 'New network broker channel UUID')
    .option(
      '--variables <json>',
      'REPLACES the whole variable set — omitted keys are deleted upstream. Omit this flag to preserve stored variables.',
    )
    .option('--draft', 'Save as DRAFT without re-running the validation script')
    .option('--output <format>', 'Output format: pretty, json, yaml', 'pretty')
    .addHelpText(
      'after',
      examples(
        `airs redteam adapter update 550e8400-... --description 'new description' --prompt 'Hello'`,
      ),
    )
    .action(async (uuid: string, opts) => {
      try {
        const fmt = opts.output as OutputFormat;
        if (fmt === 'pretty') renderRedteamHeader();
        const scriptB64 =
          opts.scriptFile !== undefined || opts.scriptB64 !== undefined
            ? resolveScriptB64(opts)
            : undefined;
        const variables = opts.variables ? parseAdapterVariables(opts.variables) : undefined;
        const service = await createService();
        if (!opts.draft && opts.channel) await assertChannelOnline(service, opts.channel);
        const updated = await service.updateAdapter(
          uuid,
          {
            prompt: opts.prompt,
            name: opts.name,
            scriptB64,
            description: opts.description,
            networkBrokerChannelUuid: opts.channel,
            variables,
          },
          opts.draft ? false : undefined,
        );
        ui.success(`Adapter updated: ${updated.uuid}`);
        renderAdapterDetail(updated, fmt);
      } catch (err) {
        fail(err);
      }
    });

  adapter
    .command('delete <uuid>')
    .description('Delete a custom target adapter')
    .option('--force', 'Skip confirmation prompt')
    .action(async (uuid: string, opts) => {
      try {
        renderRedteamHeader();
        await confirmOrAbort(`Delete adapter ${uuid}?`, Boolean(opts.force), {
          action: `delete adapter ${uuid}`,
        });
        const service = await createService();
        await service.deleteAdapter(uuid);
        ui.success(`Adapter ${uuid} deleted.`);
      } catch (err) {
        fail(err);
      }
    });

  adapter
    .command('validate')
    .description('Run an adapter script end-to-end through the broker channel without saving')
    .requiredOption('--channel <uuid>', 'Network broker channel UUID (must be ONLINE)')
    .requiredOption('--prompt <text>', 'Sample prompt to send through the adapter')
    .option('--script-file <path>', 'Path to the adapter script (encoded to base64 for you)')
    .option('--script-b64 <b64>', 'Adapter script, already base64-encoded')
    .option(
      '--variables <json>',
      'JSON array of { key, value, type } — the FULL set the script needs',
    )
    .option(
      '--adapter <uuid>',
      'Existing adapter: resolves redacted/null variable values from its stored secrets (and supplies its variable set when --variables is omitted)',
    )
    .option('--output <format>', 'Output format: pretty, json, yaml', 'pretty')
    .addHelpText(
      'after',
      examples(
        `airs redteam adapter validate --script-file ./adapter.py --channel 550e8400-... --prompt 'Hello' --variables '[{"key":"endpoint","value":"http://agent.svc:8080","type":"VAR"}]'`,
        `airs redteam adapter validate --script-file ./adapter.py --channel 550e8400-... --prompt 'Hello' --adapter 660e8400-...`,
      ),
    )
    .action(async (opts) => {
      try {
        const fmt = opts.output as OutputFormat;
        if (fmt === 'pretty') renderRedteamHeader();
        const scriptB64 = resolveScriptB64(opts);
        const variables = opts.variables ? parseAdapterVariables(opts.variables) : undefined;
        const service = await createService();
        await assertChannelOnline(service, opts.channel);
        const result = await service.validateAdapter({
          scriptB64,
          networkBrokerChannelUuid: opts.channel,
          prompt: opts.prompt,
          variables,
          adapterUuid: opts.adapter,
        });
        renderAdapterValidation(result, fmt);
        if (!result.validated) process.exitCode = 1;
      } catch (err) {
        fail(err);
      }
    });

  const networkBroker = redteam
    .command('network-broker')
    .description('Manage red team network broker channels');

  const channels = networkBroker.command('channels').description('Manage network broker channels');

  channels
    .command('list')
    .description('List network broker channels')
    .option('--limit <n>', 'Max results')
    .option('--offset <n>', 'Starting offset')
    .option('--search <text>', 'Filter by search text')
    .option('--status <status...>', 'Filter by status (ONLINE, OFFLINE, DRAFT)')
    .option('--output <format>', 'Output format: pretty, table, csv, json, yaml', 'pretty')
    .action(async (opts) => {
      try {
        const fmt = opts.output as OutputFormat;
        if (fmt === 'pretty') renderRedteamHeader();
        const service = await createService();
        const { channels: list } = await service.listChannels({
          limit: opts.limit ? parsePositiveInt(opts.limit, '--limit') : undefined,
          offset: opts.offset ? Number.parseInt(opts.offset, 10) : undefined,
          search: opts.search,
          status: opts.status,
        });
        renderChannelList(list, fmt);
      } catch (err) {
        fail(err);
      }
    });

  channels
    .command('get <channelId>')
    .description('Get a network broker channel')
    .option('--output <format>', 'Output format: pretty, json, yaml', 'pretty')
    .action(async (channelId: string, opts) => {
      try {
        const fmt = opts.output as OutputFormat;
        if (fmt === 'pretty') renderRedteamHeader();
        const service = await createService();
        const channel = await service.getChannel(channelId);
        renderChannelDetail(channel, fmt);
      } catch (err) {
        fail(err);
      }
    });

  channels
    .command('create')
    .description('Create a network broker channel')
    .requiredOption('--name <name>', 'Channel name')
    .option('--description <text>', 'Channel description')
    .action(async (opts) => {
      try {
        renderRedteamHeader();
        const service = await createService();
        const channel = await service.createChannel({
          name: opts.name,
          description: opts.description,
        });
        ui.success(`Channel created: ${channel.uuid}`);
        renderChannelDetail(channel);
      } catch (err) {
        fail(err);
      }
    });

  channels
    .command('update <channelId>')
    .description('Update a network broker channel')
    .option('--name <name>', 'New channel name')
    .option('--description <text>', 'New channel description')
    .action(async (channelId: string, opts) => {
      try {
        if (opts.name === undefined && opts.description === undefined) {
          usageError('Specify --name and/or --description to update');
        }
        renderRedteamHeader();
        const service = await createService();
        const channel = await service.updateChannel(channelId, {
          name: opts.name,
          description: opts.description,
        });
        ui.success(`Channel updated: ${channel.uuid}`);
        renderChannelDetail(channel);
      } catch (err) {
        fail(err);
      }
    });

  networkBroker
    .command('stats')
    .description('Show network broker channel statistics')
    .option('--output <format>', 'Output format: pretty, json, yaml', 'pretty')
    .action(async (opts) => {
      try {
        const fmt = opts.output as OutputFormat;
        if (fmt === 'pretty') renderRedteamHeader();
        const service = await createService();
        const stats = await service.getChannelStats();
        renderChannelStats(stats, fmt);
      } catch (err) {
        fail(err);
      }
    });

  // -------------------------------------------------------------------------
  // Languages
  // -------------------------------------------------------------------------
  redteam
    .command('languages')
    .description('List tenant languages and supported job types')
    .option('--management', 'Query the management-plane endpoint instead of the data plane')
    .option('--output <format>', 'Output format: pretty, table, csv, json, yaml', 'pretty')
    .action(async (opts) => {
      try {
        const fmt = opts.output as OutputFormat;
        if (fmt === 'pretty') renderRedteamHeader();
        const service = await createService();
        const data = await service.getLanguages(Boolean(opts.management));
        renderLanguages(data, fmt);
      } catch (err) {
        fail(err);
      }
    });
}
