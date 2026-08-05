import type { Command } from 'commander';
import { aiGatewayGrantHint, SdkAiGatewayService } from '../../airs/aigateway.js';
import type { AiGatewayPlane } from '../../airs/types.js';
import { aiGatewayClientOptions } from '../../config/client-options.js';
import { loadConfig } from '../../config/loader.js';
import { confirmOrAbort } from '../confirm.js';
import { examples } from '../examples.js';
import {
  fail,
  type OutputFormat,
  renderAiGatewayHeader,
  renderCostReport,
  renderWorkspaceDetail,
  renderWorkspaceList,
  ui,
  usageError,
} from '../renderer/index.js';

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

/**
 * Collect workspace write fields from command options. Only keys actually
 * passed are emitted, so update stays a true partial patch and create never
 * sends stray nulls. `--metadata` is sugar for `defaults.metadata` (where the
 * API really keeps it) and wins over `--defaults` on that key.
 */
export function buildWorkspaceWriteRequest(opts: Record<string, unknown>): {
  name?: string;
  description?: string;
  icon?: string;
  defaults?: Record<string, unknown>;
  users?: string[];
  usageLimits?: Array<Record<string, unknown>>;
  rateLimits?: Array<Record<string, unknown>>;
} {
  const out: Record<string, unknown> = {};
  for (const key of ['name', 'description', 'icon'] as const) {
    if (opts[key] !== undefined) out[key] = opts[key];
  }

  const defaults = parseJsonFlag(opts.defaults as string | undefined, '--defaults');
  const metadata = parseJsonFlag(opts.metadata as string | undefined, '--metadata');
  if (defaults !== undefined || metadata !== undefined) {
    out.defaults = {
      ...(typeof defaults === 'object' && defaults !== null ? defaults : {}),
      ...(metadata !== undefined ? { metadata } : {}),
    };
  }

  if (opts.users !== undefined) {
    out.users = (opts.users as string)
      .split(',')
      .map((u) => u.trim())
      .filter(Boolean);
  }

  const usage = parseJsonFlag(opts.usageLimits as string | undefined, '--usage-limits');
  if (usage !== undefined) out.usageLimits = usage;
  const rate = parseJsonFlag(opts.rateLimits as string | undefined, '--rate-limits');
  if (rate !== undefined) out.rateLimits = rate;

  return out;
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
  const aigateway = program.command('aigateway').description('AI Gateway operations');

  const workspace = aigateway.command('workspace').description('Manage AI Gateway workspaces');

  workspace
    .command('list')
    .description('List workspaces (default: active workspaces you are scoped to)')
    .option('--plane <plane>', 'Plane to read from: data (scoped) or admin (whole tenant)')
    .option('--status <status>', 'Filter by lifecycle state: active or archived')
    .option('--all', 'Merge active + archived admin-plane reads (whole tenant, both states)')
    .option('--output <format>', 'Output format: pretty, table, csv, json, yaml', 'pretty')
    .addHelpText(
      'after',
      examples(
        'airs aigateway workspace list',
        'airs aigateway workspace list --plane admin',
        'airs aigateway workspace list --plane admin --status archived',
        'airs aigateway workspace list --all --output json',
      ),
    )
    .action(async (opts) => {
      try {
        const fmt = opts.output as OutputFormat;
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

  workspace
    .command('get <ref>')
    .description('Get one workspace by UUID or slug (includes settings blocks)')
    .option('--plane <plane>', 'Plane to read from: data (scoped) or admin (whole tenant)')
    .option('--output <format>', 'Output format: pretty, json, yaml', 'pretty')
    .addHelpText(
      'after',
      examples(
        'airs aigateway workspace get ws-main-a-349e0e',
        'airs aigateway workspace get 16f7e90d-382a-4e78-b577-1b01eb5f8297 --plane admin --output json',
      ),
    )
    .action(async (ref: string, opts) => {
      try {
        const fmt = opts.output as OutputFormat;
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

  workspace
    .command('create')
    .description('Create a workspace (admin plane)')
    .requiredOption('--name <name>', 'Display name')
    .requiredOption(
      '--scope-name <scope>',
      'SCM role scope granting data-plane access (e.g. ws_production_bx7qw0) — not derived from --name',
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
        'airs aigateway workspace create --name Production --scope-name ws_production_bx7qw0',
        `airs aigateway workspace create --name Production --scope-name ws_production_bx7qw0 --metadata '{"env":"production"}' --rate-limits '[{"type":"requests","unit":"rpm","value":100}]'`,
      ),
    )
    .action(async (opts) => {
      try {
        const fmt = opts.output as OutputFormat;
        if (fmt === 'pretty') renderAiGatewayHeader();
        if (scopeNameLooksUnrelated(opts.name, opts.scopeName)) {
          ui.warn(
            `--scope-name '${opts.scopeName}' shares no token with --name '${opts.name}'. ` +
              'A workspace created with a scope nobody holds will not appear in data-plane lists.',
          );
        }
        const service = await createService();
        const workspace = await service.createWorkspace({
          ...buildWorkspaceWriteRequest(opts),
          name: opts.name,
          scopeName: opts.scopeName,
        });
        ui.success(`Workspace created: ${workspace.id}`);
        renderWorkspaceDetail(workspace, fmt);
      } catch (err) {
        failWithGrantHint(err);
      }
    });

  workspace
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
        `airs aigateway workspace update ws-produc-985697 --description 'Production workloads, us-east'`,
      ),
    )
    .action(async (ref: string, opts) => {
      try {
        const fmt = opts.output as OutputFormat;
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

  workspace
    .command('delete <ref>')
    .description('Archive a workspace (soft delete — there is no hard delete)')
    .option('--force', 'Skip confirmation prompt')
    .addHelpText('after', examples('airs aigateway workspace delete ws-produc-985697 --force'))
    .action(async (ref: string, opts) => {
      try {
        renderAiGatewayHeader();
        await confirmOrAbort(
          `Archive workspace ${ref}? (soft delete — the row remains under --status archived)`,
          Boolean(opts.force),
          { action: `archive workspace ${ref}` },
        );
        const service = await createService();
        await service.deleteWorkspace(ref);
        ui.success(`Workspace archived: ${ref}`);
        ui.status(
          'This is a soft delete — the workspace remains visible via `workspace list --plane admin --status archived`. A `get` on it now answers 404; that is expected.',
        );
      } catch (err) {
        failWithGrantHint(err);
      }
    });

  const telemetry = aigateway
    .command('telemetry')
    .description('AI Gateway runtime telemetry (data plane)');

  telemetry
    .command('cost')
    .description(
      'Total and per-day spend for a workspace (API reports cents; pretty output shows dollars)',
    )
    .requiredOption('--workspace <slug>', 'Workspace slug (not UUID), e.g. ws-main-a-349e0e')
    .option('--days <n>', 'Rolling window in days, counted back from now', '7')
    .option('--output <format>', 'Output format: pretty, json, yaml', 'pretty')
    .addHelpText(
      'after',
      examples(
        'airs aigateway telemetry cost --workspace ws-main-a-349e0e',
        'airs aigateway telemetry cost --workspace ws-main-a-349e0e --days 30 --output json',
      ),
    )
    .action(async (opts) => {
      try {
        const fmt = opts.output as OutputFormat;
        if (fmt === 'pretty') renderAiGatewayHeader();
        const days = Number.parseInt(opts.days, 10);
        if (!Number.isFinite(days) || days <= 0) {
          usageError(`Invalid --days '${opts.days}'. Expected a positive integer`);
        }
        const service = await createService();
        const report = await service.getTelemetryCost({ workspaceSlug: opts.workspace, days });
        renderCostReport(report, fmt);
      } catch (err) {
        failWithGrantHint(err);
      }
    });
}
