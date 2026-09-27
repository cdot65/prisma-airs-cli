import {
  GatewayAdminGuardrailCreateRequestSchema,
  GatewayAdminGuardrailMcpMappingRequestSchema,
  GatewayAdminGuardrailMcpSyncRequestSchema,
  GatewayGuardrailUpdateRequestSchema,
} from '@cdot65/prisma-airs-sdk';
import type { Command } from 'commander';
import { CliUsageError, fail } from '../../renderer/index.js';
import {
  addReadOutput,
  addWriteOutput,
  runConfirmedWrite,
  runDetail,
  runList,
  runWrite,
  showHelpOnEmpty,
} from './shared.js';
import {
  addStructuredInputOptions,
  buildStructuredRequest,
  type NamedRequestField,
  parseIntegerOption,
  parseJsonOption,
} from './structured-input.js';

function paginationInteger(value: string, flag: string, minimum: number, maximum: number): number {
  try {
    const parsed = parseIntegerOption(value);
    if (parsed < minimum || parsed > maximum) {
      throw new CliUsageError(`${flag} must be an integer from ${minimum} to ${maximum}`);
    }
    return parsed;
  } catch (error) {
    fail(error);
  }
}

/** Admin scope is explicit and never inferred from a workspace permission failure. */
export function registerAdminGuardrails(root: Command): void {
  const group = showHelpOnEmpty(
    root
      .command('admin-guardrails')
      .description(
        'Manage organisation guardrails on the admin plane (mutations specification-tested)',
      ),
  );
  const list = addReadOutput(
    group
      .command('list')
      .alias('ls')
      .description(
        'List organisation guardrails; optional workspace filtering may be denied by policy',
      )
      .option('--workspace <uuid>', 'Optional workspace UUID filter')
      .option('--page-size <number>', 'Page size, 1–1000', (value) =>
        paginationInteger(value, '--page-size', 1, 1000),
      )
      .option('--current-page <number>', 'Zero-based page number', (value) =>
        paginationInteger(value, '--current-page', 0, Number.MAX_SAFE_INTEGER),
      ),
  );
  list.action((opts) =>
    runList(list, opts, 'Admin guardrails', (c) =>
      c.adminGuardrails.list({
        workspaceId: opts.workspace,
        pageSize: opts.pageSize,
        currentPage: opts.currentPage,
      }),
    ),
  );
  const get = addReadOutput(
    group.command('get <id>').description('Read an admin guardrail by UUID'),
  );
  get.action((id, opts) => runDetail(get, opts, (c) => c.adminGuardrails.get(id)));
  const fields: NamedRequestField[] = [
    { option: 'name', path: 'name' },
    { option: 'checks', path: 'checks', parse: parseJsonOption },
    { option: 'actions', path: 'actions', parse: parseJsonOption },
  ];
  const common = (cmd: Command) =>
    addWriteOutput(
      addStructuredInputOptions(
        cmd
          .option('--name <name>', 'Guardrail name')
          .option('--checks <json>', 'Checks array')
          .option('--actions <json>', 'Actions object'),
      ),
    );
  const create = common(
    group
      .command('create')
      .description('Create an admin guardrail (specification-tested; no workspace fallback)'),
  )
    .option('--target <target>', 'llm or mcp_tools')
    .option('--workspace <uuid>', 'Optional workspace UUID')
    .option('--organisation <uuid>', 'Optional organisation UUID, not the numeric TSG');
  create.action((opts) =>
    runWrite(
      create,
      opts,
      () =>
        buildStructuredRequest(opts, GatewayAdminGuardrailCreateRequestSchema, [
          ...fields,
          { option: 'target', path: 'target' },
          { option: 'workspace', path: 'workspace_id' },
          { option: 'organisation', path: 'organisation_id' },
        ]),
      (c, body) => c.adminGuardrails.create(body),
    ),
  );
  const update = common(
    group.command('update <id>').description('Update an admin guardrail (specification-tested)'),
  );
  update.action((id, opts) =>
    runWrite(
      update,
      opts,
      () => buildStructuredRequest(opts, GatewayGuardrailUpdateRequestSchema, fields),
      (c, body) => c.adminGuardrails.update(id, body),
    ),
  );
  const remove = addWriteOutput(
    group
      .command('delete <id>')
      .alias('rm')
      .description('Permanently delete an admin guardrail')
      .option('--force', 'Skip confirmation prompt'),
  );
  remove.action((id, opts) =>
    runConfirmedWrite(remove, opts, `Permanently delete admin guardrail ${id}?`, (c) =>
      c.adminGuardrails.delete(id),
    ),
  );
  const mappings = showHelpOnEmpty(
    group
      .command('mcp-servers')
      .description('Manage admin guardrail MCP mappings; does not sign in to MCP'),
  );
  const mappingsList = addReadOutput(
    mappings.command('list <guardrail-id>').alias('ls').description('List MCP mappings'),
  );
  mappingsList.action((id, opts) =>
    runList(mappingsList, opts, 'MCP mappings', (c) => c.adminGuardrails.getMcpServers(id)),
  );
  const sync = addWriteOutput(
    addStructuredInputOptions(
      mappings
        .command('sync <guardrail-id>')
        .description('Replace all mappings (specification-tested)')
        .option('--force', 'Skip replacement confirmation'),
    ),
  );
  sync.action((id, opts) =>
    runConfirmedWrite(
      sync,
      opts,
      `Replace all MCP mappings on admin guardrail ${id}?`,
      () => buildStructuredRequest(opts, GatewayAdminGuardrailMcpSyncRequestSchema),
      (c, body) => c.adminGuardrails.syncMcpServers(id, body),
    ),
  );
  const upsert = addWriteOutput(
    addStructuredInputOptions(
      mappings
        .command('upsert <guardrail-id> <server-id>')
        .description('Set one mapping (specification-tested)'),
    ),
  );
  upsert.action((id, serverId, opts) =>
    runWrite(
      upsert,
      opts,
      () => buildStructuredRequest(opts, GatewayAdminGuardrailMcpMappingRequestSchema),
      (c, body) => c.adminGuardrails.upsertMcpServer(id, serverId, body),
    ),
  );
}
