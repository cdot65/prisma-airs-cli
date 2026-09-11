import {
  GatewayDefaultsInputSchema,
  GatewayRateLimitInputSchema,
  GatewayUsageLimitInputSchema,
} from '@cdot65/prisma-airs-sdk';
import type { Command } from 'commander';
import { aiGatewayGrantHint, SdkAiGatewayService } from '../../airs/aigateway.js';
import type { AiGatewayPlane, AiGatewayWorkspaceUpdateRequest } from '../../airs/types.js';
import { aiGatewayClientOptions } from '../../config/client-options.js';
import { loadConfig } from '../../config/loader.js';
import { confirmOrAbort } from '../confirm.js';
import { examples } from '../examples.js';
import {
  CliUsageError,
  fail,
  renderAiGatewayHeader,
  renderCostReport,
  renderScopeDetail,
  renderScopeList,
  renderWorkspaceDetail,
  renderWorkspaceList,
  resolveOutput,
  ui,
  usageError,
} from '../renderer/index.js';
import { registerAiGatewayInference } from './aigateway/inference.js';
import { registerAiGatewayInventory } from './aigateway/inventory.js';
import { parsePositiveInteger, registerAiGatewayTelemetryReads } from './aigateway/telemetry.js';
import { addChartFilterOptions, chartFiltersFrom } from './aigateway/telemetry-filters.js';
import { registerAiGatewayReportCommand } from './aigateway-report.js';

/** Create an SdkAiGatewayService from config. */
async function createService() {
  const config = await loadConfig();
  return new SdkAiGatewayService(aiGatewayClientOptions(config));
}

/** fail(), prefixed with the grant hint when the error is an AI Gateway 403. */
function failWithGrantHint(err: unknown): never {
  const hint = aiGatewayGrantHint(err);
  if (hint) ui.warn(`403: ${hint}`);
  fail(err);
}

function parsePlane(value: string | undefined): AiGatewayPlane | undefined {
  if (value === undefined) return undefined;
  if (value !== 'data' && value !== 'admin') {
    usageError(`Invalid --plane '${value}'. Valid planes: data, admin`);
  }
  return value;
}

function parseStatus(value: string | undefined): 'active' | 'archived' | undefined {
  if (value === undefined) return undefined;
  if (value !== 'active' && value !== 'archived') {
    usageError(`Invalid --status '${value}'. Valid statuses: active, archived`);
  }
  return value;
}

function parseJsonFlag(raw: string | undefined, flag: string): unknown {
  if (raw === undefined) return undefined;
  try {
    return JSON.parse(raw);
  } catch {
    throw new Error(`${flag} must be valid JSON`);
  }
}

type WorkspaceWriteRequest = Pick<
  AiGatewayWorkspaceUpdateRequest,
  'name' | 'description' | 'icon' | 'defaults' | 'usageLimits' | 'rateLimits'
> & { users?: string[] };

/**
 * Collect workspace write fields from command options. Only keys actually
 * passed are emitted, so update stays a true partial patch and create never
 * sends stray nulls. `--metadata` is sugar for `defaults.metadata` (where the
 * API really keeps it) and wins over `--defaults` on that key.
 */
export function buildWorkspaceWriteRequest(opts: Record<string, unknown>): WorkspaceWriteRequest {
  const out: Record<string, unknown> = {};
  for (const key of ['name', 'description', 'icon'] as const) {
    if (opts[key] !== undefined) out[key] = opts[key];
  }

  const defaults = parseJsonFlag(opts.defaults as string | undefined, '--defaults');
  const metadata = parseJsonFlag(opts.metadata as string | undefined, '--metadata');
  if (defaults !== undefined || metadata !== undefined) {
    out.defaults = GatewayDefaultsInputSchema.parse({
      ...(typeof defaults === 'object' && defaults !== null ? defaults : {}),
      ...(metadata !== undefined ? { metadata } : {}),
    });
  }

  if (opts.users !== undefined) {
    out.users = (opts.users as string)
      .split(',')
      .map((u) => u.trim())
      .filter(Boolean);
  }

  const usage = parseJsonFlag(opts.usageLimits as string | undefined, '--usage-limits');
  if (usage !== undefined) out.usageLimits = GatewayUsageLimitInputSchema.array().parse(usage);
  const rate = parseJsonFlag(opts.rateLimits as string | undefined, '--rate-limits');
  if (rate !== undefined) out.rateLimits = GatewayRateLimitInputSchema.array().parse(rate);

  return out as WorkspaceWriteRequest;
}

/**
 * `scope_name` is not derived from `name`, and a workspace created with a
 * scope nobody holds silently vanishes from data-plane lists — the most common
 * way a fresh workspace "goes missing". Heuristic: flag a scope sharing no
 * token with the workspace name. Names too short to compare are never flagged.
 */
export function scopeNameLooksUnrelated(name: string, scopeName: string): boolean {
  const nameToken = name.toLowerCase().replace(/[^a-z0-9]/g, '');
  if (nameToken.length < 4) return false;
  return !scopeName
    .toLowerCase()
    .replace(/[^a-z0-9]/g, '')
    .includes(nameToken);
}

/** Register the `aigateway` command group. */
export function registerAiGatewayCommand(program: Command): void {
  const aigateway = program
    .command('aigateway')
    .description('Manage and observe Prisma AIRS AI Gateway resources')
    .action(() => aigateway.outputHelp());

  registerAiGatewayInventory(aigateway);
  registerAiGatewayReportCommand(aigateway);
  registerAiGatewayInference(aigateway);

  const workspace = aigateway
    .command('workspaces')
    .alias('workspace')
    .description('Manage gateway workspaces')
    .action(() => workspace.outputHelp());

  const workspaceList = workspace
    .command('list')
    .description('List workspaces (default: active workspaces you are scoped to)')
    .option('--plane <plane>', 'Plane to read from: data (scoped) or admin (whole tenant)')
    .option('--status <status>', 'Filter by lifecycle state: active or archived')
    .option('--all', 'Merge active + archived admin-plane reads (whole tenant, both states)')
    .option('--output <format>', 'Output format: pretty, table, markdown, csv, json, yaml')
    .addHelpText(
      'after',
      examples(
        'airs aigateway workspaces list',
        'airs aigateway workspaces list --plane admin',
        'airs aigateway workspaces list --plane admin --status archived',
        'airs aigateway workspaces list --all --output json',
      ),
    )
    .action(async (opts) => {
      try {
        const fmt = await resolveOutput(workspaceList, opts);
        if (fmt === 'pretty') renderAiGatewayHeader();
        const plane = parsePlane(opts.plane);
        const status = parseStatus(opts.status);
        if (opts.all && (plane !== undefined || status !== undefined)) {
          usageError('--all already merges admin-plane active + archived; drop --plane/--status');
        }
        const service = await createService();
        const workspaces = opts.all
          ? await service.listAllWorkspaces()
          : await service.listWorkspaces(
              plane !== undefined || status !== undefined ? { plane, status } : undefined,
            );
        renderWorkspaceList(workspaces, fmt);
        if (fmt === 'pretty' && !opts.all && plane !== 'admin') {
          ui.status(
            'Data-plane list shows only active workspaces you are scoped to — use --plane admin or --all for the whole tenant.',
          );
        }
      } catch (err) {
        failWithGrantHint(err);
      }
    });

  const workspaceGet = workspace
    .command('get <ref>')
    .description('Get one workspace by UUID or slug (includes settings blocks)')
    .option('--plane <plane>', 'Plane to read from: data (scoped) or admin (whole tenant)')
    .option('--output <format>', 'Output format: pretty, table, markdown, csv, json, yaml')
    .addHelpText(
      'after',
      examples(
        'airs aigateway workspaces get ws-main-a-349e0e',
        'airs aigateway workspaces get 16f7e90d-382a-4e78-b577-1b01eb5f8297 --plane admin --output json',
      ),
    )
    .action(async (ref: string, opts) => {
      try {
        const fmt = await resolveOutput(workspaceGet, opts);
        if (fmt === 'pretty') renderAiGatewayHeader();
        const plane = parsePlane(opts.plane);
        const service = await createService();
        const workspace = await service.getWorkspace(
          ref,
          plane !== undefined ? { plane } : undefined,
        );
        renderWorkspaceDetail(workspace, fmt);
      } catch (err) {
        failWithGrantHint(err);
      }
    });

  const workspaceCreate = workspace
    .command('create')
    .description('Create a workspace: IAM scope → workspace → bind scope (admin plane)')
    .requiredOption('--name <name>', 'Display name')
    .option(
      '--scope-name <scope>',
      'IAM scope to create and bind (e.g. ws_production_bx7qw0). Default: ws_<name>_<suffix>',
    )
    .option(
      '--existing-scope',
      'Bind an IAM scope that already exists instead of creating one (requires --scope-name)',
    )
    .option('--description <text>', 'Workspace description')
    .option('--icon <icon>', 'Workspace icon')
    .option('--metadata <json>', 'Sugar for defaults.metadata (flat string map)')
    .option('--defaults <json>', 'Workspace defaults object')
    .option('--users <ids>', 'Comma-separated user ids to seed the workspace with')
    .option('--usage-limits <json>', 'Usage-limit policies — a JSON ARRAY of policy objects')
    .option('--rate-limits <json>', 'Rate-limit policies — a JSON ARRAY of policy objects')
    .option('--output <format>', 'Output format: pretty, json, yaml', 'pretty')
    .addHelpText(
      'after',
      examples(
        'airs aigateway workspaces create --name truffles --description "Online recipe generation application"',
        'airs aigateway workspaces create --name Production --scope-name ws_production_bx7qw0',
        'airs aigateway workspaces create --name Staging --scope-name ws_staging_q1x8mz --existing-scope',
        `airs aigateway workspaces create --name Production --metadata '{"env":"production"}' --rate-limits '[{"type":"requests","unit":"rpm","value":100}]'`,
      ),
    )
    .action(async (opts) => {
      try {
        // The root program also declares --output, so a raw opts.output never sees
        // `--output json` given after the subcommand; resolve it like the reads do.
        const fmt = await resolveOutput(workspaceCreate, opts, {
          allowed: ['pretty', 'json', 'yaml'],
        });
        if (fmt === 'pretty') renderAiGatewayHeader();
        if (opts.existingScope && !opts.scopeName) {
          throw new CliUsageError('--existing-scope requires --scope-name');
        }
        if (opts.scopeName && scopeNameLooksUnrelated(opts.name, opts.scopeName)) {
          ui.warn(
            `--scope-name '${opts.scopeName}' shares no token with --name '${opts.name}'. ` +
              'Operators find workspaces by scope in SCM Access Management; an unrelated name is easy to lose.',
          );
        }
        const service = await createService();
        const { workspace, scope, scopeCreated } = await service.createWorkspace({
          ...buildWorkspaceWriteRequest(opts),
          name: opts.name,
          ...(opts.scopeName !== undefined ? { scopeName: opts.scopeName } : {}),
          ...(opts.existingScope ? { existingScope: true } : {}),
        });
        ui.success(`Workspace created: ${workspace.id}`);
        ui.status(
          `IAM scope ${scope.name} ${scopeCreated ? 'created and ' : ''}bound to workspace ${workspace.slug}. ` +
            'Grant that scope to the service accounts that should reach the workspace on the data plane.',
        );
        renderWorkspaceDetail(workspace, fmt);
      } catch (err) {
        failWithGrantHint(err);
      }
    });

  const workspaceUpdate = workspace
    .command('update <ref>')
    .description('Update a workspace (admin plane, partial patch)')
    .option('--name <name>', 'New display name')
    .option('--description <text>', 'New description')
    .option('--icon <icon>', 'New icon')
    .option('--metadata <json>', 'Sugar for defaults.metadata (flat string map)')
    .option('--defaults <json>', 'Workspace defaults object')
    .option('--usage-limits <json>', 'Usage-limit policies — a JSON ARRAY of policy objects')
    .option('--rate-limits <json>', 'Rate-limit policies — a JSON ARRAY of policy objects')
    .option('--output <format>', 'Output format: pretty, json, yaml', 'pretty')
    .addHelpText(
      'after',
      examples(
        `airs aigateway workspaces update ws-produc-985697 --description 'Production workloads, us-east'`,
      ),
    )
    .action(async (ref: string, opts) => {
      try {
        const fmt = await resolveOutput(workspaceUpdate, opts, {
          allowed: ['pretty', 'json', 'yaml'],
        });
        if (fmt === 'pretty') renderAiGatewayHeader();
        const request = buildWorkspaceWriteRequest(opts);
        if (Object.keys(request).length === 0) {
          usageError(
            'Specify at least one of --name --description --icon --metadata --defaults --usage-limits --rate-limits',
          );
        }
        const service = await createService();
        const workspace = await service.updateWorkspace(ref, request);
        ui.success(`Workspace updated: ${workspace.id}`);
        renderWorkspaceDetail(workspace, fmt);
      } catch (err) {
        failWithGrantHint(err);
      }
    });

  const archiveWorkspace = async (ref: string, opts: { force?: boolean }, deprecated: boolean) => {
    try {
      renderAiGatewayHeader();
      if (deprecated) {
        ui.warn(
          '`aigateway workspace delete` is deprecated because this operation archives; use `aigateway workspaces archive`.',
        );
      }
      await confirmOrAbort(
        `Archive workspace ${ref}? (soft delete — the row remains under --status archived)`,
        Boolean(opts.force),
        { action: `archive workspace ${ref}` },
      );
      const service = await createService();
      await service.deleteWorkspace(ref);
      ui.success(`Workspace archived: ${ref}`);
      ui.status(
        'This is a soft delete — the workspace remains visible via `workspaces list --plane admin --status archived`. A `get` on it now answers 404; that is expected.',
      );
    } catch (err) {
      failWithGrantHint(err);
    }
  };

  workspace
    .command('archive <ref>')
    .description('Archive a workspace (soft delete — there is no hard delete)')
    .option('--force', 'Skip confirmation prompt')
    .addHelpText('after', examples('airs aigateway workspaces archive ws-produc-985697 --force'))
    .action((ref: string, opts) => archiveWorkspace(ref, opts, false));

  workspace
    .command('delete <ref>', { hidden: true })
    .description('Deprecated compatibility command for archive')
    .option('--force', 'Skip confirmation prompt')
    .action((ref: string, opts) => archiveWorkspace(ref, opts, true));

  const scopes = aigateway
    .command('scopes')
    .description('Manage SCM IAM scopes — the objects a workspace scope_name points at (/iam/v1)')
    .action(() => scopes.outputHelp());

  const scopeList = scopes
    .command('list')
    .description('List every IAM scope in the tenant (unbound scopes have no resources)')
    .option('--output <format>', 'Output format: pretty, table, markdown, csv, json, yaml')
    .addHelpText(
      'after',
      examples('airs aigateway scopes list', 'airs aigateway scopes list --output json'),
    )
    .action(async (opts) => {
      try {
        const fmt = await resolveOutput(scopeList, opts);
        if (fmt === 'pretty') renderAiGatewayHeader();
        const service = await createService();
        renderScopeList(await service.listScopes(), fmt);
      } catch (err) {
        failWithGrantHint(err);
      }
    });

  const scopeGet = scopes
    .command('get <name>')
    .description('Get one IAM scope by name')
    .option('--output <format>', 'Output format: pretty, table, markdown, csv, json, yaml')
    .addHelpText('after', examples('airs aigateway scopes get ws_production_bx7qw0'))
    .action(async (name: string, opts) => {
      try {
        const fmt = await resolveOutput(scopeGet, opts);
        if (fmt === 'pretty') renderAiGatewayHeader();
        const service = await createService();
        renderScopeDetail(await service.getScope(name), fmt);
      } catch (err) {
        failWithGrantHint(err);
      }
    });

  const scopeCreate = scopes
    .command('create')
    .description('Create an unbound IAM scope (step 1 of provisioning, on its own)')
    .requiredOption('--name <name>', 'Scope name, e.g. ws_production_bx7qw0')
    .option('--description <text>', 'Scope description')
    .option('--output <format>', 'Output format: pretty, json, yaml', 'pretty')
    .addHelpText(
      'after',
      examples(
        `airs aigateway scopes create --name ws_production_bx7qw0 --description 'All production applications'`,
      ),
    )
    .action(async (opts) => {
      try {
        const fmt = await resolveOutput(scopeCreate, opts, { allowed: ['pretty', 'json', 'yaml'] });
        if (fmt === 'pretty') renderAiGatewayHeader();
        const service = await createService();
        const scope = await service.createScope({
          name: opts.name,
          ...(opts.description !== undefined ? { description: opts.description } : {}),
        });
        ui.success(`IAM scope created: ${scope.name}`);
        ui.status(
          'The scope is not bound to anything yet — `workspaces create --existing-scope` or `scopes bind` finishes the job.',
        );
        renderScopeDetail(scope, fmt);
      } catch (err) {
        failWithGrantHint(err);
      }
    });

  const scopeBind = scopes
    .command('bind <name>')
    .description('Bind a workspace to an existing IAM scope (step 3 of provisioning, on its own)')
    .requiredOption('--workspace <ref>', 'Workspace slug, UUID, or unique display name')
    .option('--output <format>', 'Output format: pretty, json, yaml', 'pretty')
    .addHelpText(
      'after',
      examples('airs aigateway scopes bind ws_production_bx7qw0 --workspace ws-produc-985697'),
    )
    .action(async (name: string, opts) => {
      try {
        const fmt = await resolveOutput(scopeBind, opts, { allowed: ['pretty', 'json', 'yaml'] });
        if (fmt === 'pretty') renderAiGatewayHeader();
        const service = await createService();
        const scope = await service.bindScope(name, opts.workspace);
        ui.success(`IAM scope ${scope.name} now binds ${scope.resources.length} resource(s)`);
        renderScopeDetail(scope, fmt);
      } catch (err) {
        failWithGrantHint(err);
      }
    });

  scopes
    .command('delete <name>')
    .alias('rm')
    .description('Delete an IAM scope by name (not live-verified upstream; the API may decline)')
    .option('--force', 'Skip confirmation prompt')
    .addHelpText('after', examples('airs aigateway scopes delete ws_truffles_ggolfu --force'))
    .action(async (name: string, opts) => {
      try {
        renderAiGatewayHeader();
        await confirmOrAbort(
          `Delete IAM scope ${name}? Workspaces bound to it lose their data-plane grant.`,
          Boolean(opts.force),
          { action: `delete IAM scope ${name}` },
        );
        const service = await createService();
        await service.deleteScope(name);
        ui.success(`IAM scope deleted: ${name}`);
      } catch (err) {
        failWithGrantHint(err);
      }
    });

  const telemetry = aigateway
    .command('telemetry')
    .description('AI Gateway runtime telemetry (data plane)')
    .action(() => telemetry.outputHelp());

  registerAiGatewayTelemetryReads(telemetry);

  const cost = addChartFilterOptions(
    telemetry
      .command('cost')
      .description(
        'Total and per-day spend for a workspace (API reports cents; pretty output shows dollars)',
      )
      .requiredOption('--workspace <ref>', 'Workspace slug, UUID, or unique display name')
      .option('--days <n>', 'Rolling window in days, counted back from now', '7')
      .option('--output <format>', 'Output format: pretty, table, markdown, csv, json, yaml')
      .addHelpText(
        'after',
        examples(
          'airs aigateway telemetry cost --workspace ws-main-a-349e0e',
          'airs aigateway telemetry cost --workspace ws-main-a-349e0e --days 30 --output json',
        ),
      ),
  ).action(async (opts) => {
    try {
      const days = parsePositiveInteger(opts.days, '--days');
      const filters = chartFiltersFrom(opts);
      const fmt = await resolveOutput(cost, opts);
      if (fmt === 'pretty') renderAiGatewayHeader();
      const service = await createService();
      const report = await service.getTelemetryCost({
        workspaceSlug: opts.workspace,
        days,
        ...filters,
      });
      renderCostReport(report, fmt);
    } catch (err) {
      failWithGrantHint(err);
    }
  });
}
