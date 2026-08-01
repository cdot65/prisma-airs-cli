import type { Command } from 'commander';
import { aiGatewayGrantHint, SdkAiGatewayService } from '../../airs/aigateway.js';
import type { AiGatewayPlane } from '../../airs/types.js';
import { aiGatewayClientOptions } from '../../config/client-options.js';
import { loadConfig } from '../../config/loader.js';
import { examples } from '../examples.js';
import {
  fail,
  type OutputFormat,
  renderAiGatewayHeader,
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
}
