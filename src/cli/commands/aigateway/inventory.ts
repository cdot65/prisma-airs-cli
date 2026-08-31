import {
  AI_GATEWAY_DEPLOYMENT_STATUSES,
  AI_GATEWAY_DEPLOYMENT_TYPES,
  AI_GATEWAY_KNOWN_API_KEY_SCOPES,
  AI_GATEWAY_KNOWN_MCP_AUTH_TYPES,
  AI_GATEWAY_KNOWN_MCP_TRANSPORTS,
  AI_GATEWAY_MUTABLE_MCP_CAPABILITY_TYPES,
  type AIGatewayClient,
  type AIGatewaySecretOperation,
  GatewayApiKeyRotateRequestSchema,
  GatewayApiKeyUpdateRequestSchema,
  GatewayConfigCreateRequestSchema,
  GatewayConfigUpdateRequestSchema,
  GatewayDeploymentCreateRequestSchema,
  GatewayDeploymentUpdateRequestSchema,
  GatewayGuardrailCreateRequestSchema,
  GatewayGuardrailUpdateRequestSchema,
  GatewayIntegrationCreateRequestSchema,
  GatewayIntegrationModelsBulkUpdateRequestSchema,
  GatewayIntegrationUpdateRequestSchema,
  GatewayIntegrationWorkspacesBulkUpdateRequestSchema,
  GatewayOrganisationAuthSettingsUpdateRequestSchema,
  GatewayOrganisationUpdateRequestSchema,
  GatewayPluginCreateRequestSchema,
  GatewayProviderCreateRequestSchema,
  GatewayProviderUpdateRequestSchema,
  GatewayServiceApiKeyCreateRequestSchema,
  GatewayUserApiKeyCreateRequestSchema,
  McpIntegrationCapabilitiesBulkUpdateRequestSchema,
  McpIntegrationCreateRequestSchema,
  McpIntegrationUpdateRequestSchema,
  McpIntegrationWorkspacesBulkUpdateRequestSchema,
  redactAIGatewaySecrets,
} from '@cdot65/prisma-airs-sdk';
import type { Command } from 'commander';
import { redactDeep } from '../../debug-logger.js';
import { CliUsageError } from '../../renderer/index.js';
import {
  addReadOutput,
  addWriteOutput,
  runConfirmedWrite,
  runDetail,
  runList,
  runSecretWrite,
  runWrite,
  showHelpOnEmpty,
} from './shared.js';
import {
  addStructuredInputOptions,
  buildStructuredRequest,
  collectOption,
  type NamedRequestField,
  parseBooleanBindingsOption,
  parseBooleanOption,
  parseCapabilityBindingsOption,
  parseCsvOption,
  parseDateOption,
  parseIntegerOption,
  parseJsonOption,
  parseModelBindingsOption,
  parseStringMapOption,
} from './structured-input.js';

type ScopedClient = {
  list(options: { workspaceId: string }): Promise<unknown>;
  get(id: string): Promise<unknown>;
};

function registerScopedReads(
  root: Command,
  name: string,
  description: string,
  select: (client: AIGatewayClient) => ScopedClient,
  options: {
    sensitiveDetail?: boolean;
    sensitiveOperation?: AIGatewaySecretOperation;
  } = {},
): Command {
  const group = showHelpOnEmpty(root.command(name).description(description));
  const list = addReadOutput(
    group
      .command('list')
      .description(`List ${name} in a workspace (data plane)`)
      .requiredOption('--workspace <uuid>', 'Workspace UUID'),
  );
  list.action((opts) =>
    runList(list, opts, name, (client) => select(client).list({ workspaceId: opts.workspace })),
  );
  let get = group
    .command('get <id>')
    .description(`Get one ${name.replace(/s$/, '')} by UUID (data plane)`);
  if (options.sensitiveDetail) {
    get = get.option('--reveal-sensitive', 'Show credential-bearing fields');
  }
  addReadOutput(get);
  get.action((id, opts) =>
    runDetail(get, opts, async (client) => {
      const result = await select(client).get(id);
      if (!options.sensitiveDetail || opts.revealSensitive) return result;
      const metadataRedacted = options.sensitiveOperation
        ? redactAIGatewaySecrets(options.sensitiveOperation, result, 'response')
        : result;
      return redactDeep(metadataRedacted);
    }),
  );
  return group;
}

interface RequestSchema<T> {
  parse(value: unknown): T;
}

const knownValues = (values: readonly string[]): string => values.join(', ');

function redactApiKeyMaterial(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(redactApiKeyMaterial);
  if (value === null || typeof value !== 'object') return value;
  return Object.fromEntries(
    Object.entries(value).map(([key, entry]) => [
      key,
      key === 'key' ? '***' : redactApiKeyMaterial(entry),
    ]),
  );
}

function parseNamedDate(value: unknown, flag: string): Date {
  try {
    return parseDateOption(value);
  } catch (error) {
    throw new CliUsageError(
      `Invalid ${flag}: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
}

function parsePositiveInteger(value: unknown, flag: string): number {
  try {
    const parsed = parseIntegerOption(value);
    if (parsed <= 0) throw new CliUsageError('Expected a positive integer');
    return parsed;
  } catch (error) {
    throw new CliUsageError(
      `Invalid ${flag}: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
}

function addCrudMutationNodes<TCreate, TUpdate>(
  group: Command,
  resource: string,
  request: {
    createFields?: readonly NamedRequestField[];
    createOptions?: (command: Command) => Command;
    createSchema: RequestSchema<TCreate>;
    updateFields?: readonly NamedRequestField[];
    updateOptions?: (command: Command) => Command;
    updateSchema: RequestSchema<TUpdate>;
  },
  operations: {
    create(client: AIGatewayClient, body: TCreate): Promise<unknown>;
    delete(client: AIGatewayClient, id: string): Promise<unknown>;
    update(client: AIGatewayClient, id: string, body: TUpdate): Promise<unknown>;
  },
): void {
  let create = group.command('create').description(`Create a ${resource} from structured flags`);
  if (request.createOptions) create = request.createOptions(create);
  create = addWriteOutput(addStructuredInputOptions(create));
  create.action((opts) =>
    runWrite(
      create,
      opts,
      () => buildStructuredRequest(opts, request.createSchema, request.createFields),
      (client, body) => operations.create(client, body),
    ),
  );

  let update = group
    .command('update <id>')
    .description(`Update a ${resource} with structured flags`);
  if (request.updateOptions) update = request.updateOptions(update);
  update = addWriteOutput(addStructuredInputOptions(update));
  update.action((id, opts) =>
    runWrite(
      update,
      opts,
      () => buildStructuredRequest(opts, request.updateSchema, request.updateFields),
      (client, body) => operations.update(client, id, body),
    ),
  );

  const remove = addWriteOutput(
    group
      .command('delete <id>')
      .description('Permanently delete this resource')
      .option('--force', 'Skip confirmation prompt'),
  );
  remove.action((id, opts) =>
    runConfirmedWrite(remove, opts, `Permanently delete ${resource} ${id}?`, (client) =>
      operations.delete(client, id),
    ),
  );
}

function registerApiKeys(root: Command): void {
  const apiKeys = showHelpOnEmpty(
    root.command('api-keys').description('Manage service and user gateway credentials'),
  );
  for (const kind of ['service', 'user'] as const) {
    const group = showHelpOnEmpty(apiKeys.command(kind).description(`Manage ${kind} API keys`));
    const list = addReadOutput(
      group
        .command('list')
        .description(`List ${kind} API keys in a workspace (data plane)`)
        .requiredOption('--workspace <uuid>', 'Workspace UUID')
        .option('--reveal-sensitive', 'Show API key material'),
    );
    list.action((opts) =>
      runList(list, opts, `${kind} API keys`, async (client) => {
        const result = await (kind === 'service'
          ? client.apiKeys.listService({ workspaceId: opts.workspace })
          : client.apiKeys.listUser({ workspaceId: opts.workspace }));
        return opts.revealSensitive ? result : redactApiKeyMaterial(result);
      }),
    );
    const get = addReadOutput(
      group
        .command('get <id>')
        .description(`Get one ${kind} API key`)
        .option('--reveal-sensitive', 'Show API key material'),
    );
    get.action((id, opts) =>
      runDetail(get, opts, async (client) => {
        const result = await (kind === 'service'
          ? client.apiKeys.getService(id)
          : client.apiKeys.getUser(id));
        return opts.revealSensitive ? result : redactApiKeyMaterial(result);
      }),
    );
    const createFields: NamedRequestField[] = [
      { option: 'alertEmails', path: 'alert_emails', parse: parseCsvOption },
      { option: 'description', path: 'description' },
      { option: 'expiresAt', path: 'expires_at' },
      { option: 'name', path: 'name' },
      { option: 'organisationId', path: 'organisation_id' },
      { option: 'scopes', path: 'scopes', parse: parseCsvOption },
      { option: 'type', path: 'type' },
      { option: 'workspace', path: 'workspace_id' },
      ...(kind === 'user' ? [{ option: 'userId', path: 'user_id' }] : []),
    ];
    let createCommand = group
      .command('create')
      .description(`Create a ${kind} API key from structured flags`)
      .option('--alert-emails <emails>', 'Comma-separated alert email addresses')
      .option('--description <text>', 'Credential description')
      .option('--expires-at <iso>', 'Expiration as ISO-8601')
      .option('--name <name>', 'Credential name')
      .option('--organisation-id <tsg>', 'Numeric TSG id')
      .option(
        '--scopes <scopes>',
        `Comma-separated scopes (known: ${knownValues(AI_GATEWAY_KNOWN_API_KEY_SCOPES)})`,
      )
      .option('--type <type>', 'Credential type')
      .option('--workspace <uuid>', 'Workspace UUID')
      .option('--secret-output <path>', 'Write the one-time credential to a new 0600 file')
      .option('--show-secret', 'Print the one-time credential to stdout');
    if (kind === 'user') createCommand = createCommand.option('--user-id <uuid>', 'User UUID');
    const create = addWriteOutput(addStructuredInputOptions(createCommand));
    create.action((opts) => {
      if (kind === 'service') {
        return runSecretWrite(
          create,
          opts,
          () => buildStructuredRequest(opts, GatewayServiceApiKeyCreateRequestSchema, createFields),
          (client, body) => client.apiKeys.createService(body),
        );
      }
      return runSecretWrite(
        create,
        opts,
        () => buildStructuredRequest(opts, GatewayUserApiKeyCreateRequestSchema, createFields),
        (client, body) => client.apiKeys.createUser(body),
      );
    });

    const update = addWriteOutput(
      addStructuredInputOptions(
        group
          .command('update <id>')
          .description(`Update a ${kind} API key with structured flags`)
          .option('--alert-emails <emails>', 'Comma-separated alert email addresses')
          .option('--description <text>', 'Credential description')
          .option('--expires-at <iso>', 'Expiration as ISO-8601')
          .option('--name <name>', 'Credential name')
          .option('--reset-usage <boolean>', 'Reset accumulated usage: true or false')
          .option(
            '--scopes <scopes>',
            `Comma-separated scopes (known: ${knownValues(AI_GATEWAY_KNOWN_API_KEY_SCOPES)})`,
          ),
      ),
    );
    update.action((id, opts) =>
      runWrite(
        update,
        opts,
        () =>
          buildStructuredRequest(opts, GatewayApiKeyUpdateRequestSchema, [
            { option: 'alertEmails', path: 'alert_emails', parse: parseCsvOption },
            { option: 'description', path: 'description' },
            { option: 'expiresAt', path: 'expires_at' },
            { option: 'name', path: 'name' },
            { option: 'resetUsage', path: 'reset_usage', parse: parseBooleanOption },
            { option: 'scopes', path: 'scopes', parse: parseCsvOption },
          ]),
        (client, body) =>
          kind === 'service'
            ? client.apiKeys.updateService(id, body)
            : client.apiKeys.updateUser(id, body),
      ),
    );

    const remove = addWriteOutput(
      group
        .command('delete <id>')
        .description(`Revoke a ${kind} API key`)
        .option('--force', 'Skip confirmation prompt'),
    );
    remove.action((id, opts) =>
      runConfirmedWrite(remove, opts, `Revoke ${kind} API key ${id}?`, (client) =>
        kind === 'service' ? client.apiKeys.deleteService(id) : client.apiKeys.deleteUser(id),
      ),
    );

    const rotate = addWriteOutput(
      group
        .command('rotate <id>')
        .description(`Rotate a ${kind} API key`)
        .option('--force', 'Skip confirmation prompt')
        .option('--secret-output <path>', 'Write the one-time credential to a new 0600 file')
        .option('--show-secret', 'Print the one-time credential to stdout')
        .option('--transition-ms <ms>', 'Credential overlap in milliseconds'),
    );
    rotate.action((id, opts) =>
      runSecretWrite(
        rotate,
        opts,
        () =>
          buildStructuredRequest(opts, GatewayApiKeyRotateRequestSchema, [
            {
              option: 'transitionMs',
              path: 'key_transition_period_ms',
              parse: parseIntegerOption,
            },
          ]),
        (client, body) =>
          kind === 'service'
            ? client.apiKeys.rotateService(id, body)
            : client.apiKeys.rotateUser(id, body),
        `Rotate ${kind} API key ${id}?`,
      ),
    );
  }
}

function registerAuditLogs(root: Command): void {
  const group = showHelpOnEmpty(
    root.command('audit-logs').description('Inspect organisation audit activity'),
  );
  const list = addReadOutput(
    group
      .command('list')
      .description('List audit activity for a UTC time window (admin plane)')
      .option('--days <n>', 'Rolling window in days', '7')
      .option('--end <iso>', 'Window end as ISO-8601')
      .option('--reveal-sensitive', 'Show sensitive request fields')
      .option('--start <iso>', 'Window start as ISO-8601'),
  );
  list.action((opts) => {
    return runList(list, opts, 'audit logs', async (client) => {
      const end = opts.end ? parseNamedDate(opts.end, '--end') : new Date();
      const start = opts.start
        ? parseNamedDate(opts.start, '--start')
        : new Date(end.getTime() - parsePositiveInteger(opts.days, '--days') * 86_400_000);
      const result = await client.auditLogs.list({ start, end });
      return opts.revealSensitive ? result : redactDeep(result);
    });
  });
}

function registerConfigs(root: Command): void {
  const group = registerScopedReads(
    root,
    'configs',
    'Manage routing configurations',
    (client) => client.configs,
  );
  const versions = addReadOutput(
    group.command('versions <id>').description('List immutable config versions'),
  );
  versions.action((id, opts) =>
    runList(versions, opts, 'config versions', (client) => client.configs.listVersions(id)),
  );
  const commonFields: NamedRequestField[] = [
    { option: 'name', path: 'name' },
    { option: 'workspace', path: 'workspace_id' },
    { option: 'status', path: 'status' },
  ];
  addCrudMutationNodes(
    group,
    'config',
    {
      createFields: commonFields,
      createOptions: (command) =>
        command
          .option('--name <name>', 'Config name')
          .option('--workspace <uuid>', 'Workspace UUID'),
      createSchema: GatewayConfigCreateRequestSchema,
      updateFields: commonFields,
      updateOptions: (command) =>
        command
          .option('--name <name>', 'New config name')
          .option('--status <status>', 'New config status')
          .option('--workspace <uuid>', 'New workspace UUID'),
      updateSchema: GatewayConfigUpdateRequestSchema,
    },
    {
      create: (client, body) => client.configs.create(body),
      delete: (client, id) => client.configs.delete(id),
      update: (client, id, body) => client.configs.update(id, body),
    },
  );
}

function registerDeployments(root: Command): void {
  const group = showHelpOnEmpty(
    root.command('deployments').description('Manage self-hosted gateway registrations'),
  );
  const list = addReadOutput(group.command('list').description('List deployments (admin plane)'));
  list.action((opts) => runList(list, opts, 'deployments', (client) => client.deployments.list()));
  const get = addReadOutput(
    group.command('get <id>').description('Get a deployment by UUID (admin plane)'),
  );
  get.action((id, opts) => runDetail(get, opts, (client) => client.deployments.get(id)));
  const ping = addReadOutput(
    group.command('ping <id>').description('Run the optional control-plane ingress diagnostic'),
  );
  ping.action((id, opts) => runDetail(ping, opts, (client) => client.deployments.ping(id)));
  const archive = addWriteOutput(
    group
      .command('archive <id>')
      .description('Archive a deployment registration')
      .requiredOption('--organisation-id <tsg>', 'Numeric TSG id')
      .option('--force', 'Skip confirmation prompt'),
  );
  archive.action((id, opts) =>
    runConfirmedWrite(archive, opts, `Archive deployment ${id}?`, (client) =>
      client.deployments.delete(id, opts.organisationId),
    ),
  );
  const create = addWriteOutput(
    addStructuredInputOptions(
      group
        .command('create')
        .description('Register a self-hosted deployment from structured flags')
        .option('--auth-settings <json>', 'Deployment authentication settings object')
        .option('--deployment-config <json>', 'Deployment configuration object')
        .option('--is-default <boolean>', 'Make this the default deployment')
        .option('--name <name>', 'Deployment name')
        .option('--organisation-id <tsg>', 'Numeric TSG id')
        .option('--slug <slug>', 'Stable deployment slug')
        .option('--type <type>', `Deployment type: ${knownValues(AI_GATEWAY_DEPLOYMENT_TYPES)}`)
        .option('--secret-output <path>', 'Write registration credentials to a new 0600 file')
        .option('--show-secret', 'Print registration credentials to stdout'),
    ),
  );
  create.action((opts) =>
    runSecretWrite(
      create,
      opts,
      () =>
        buildStructuredRequest(opts, GatewayDeploymentCreateRequestSchema, [
          { option: 'authSettings', path: 'auth_settings', parse: parseJsonOption },
          { option: 'deploymentConfig', path: 'deployment_config', parse: parseJsonOption },
          { option: 'isDefault', path: 'is_default', parse: parseBooleanOption },
          { option: 'name', path: 'name' },
          { option: 'organisationId', path: 'organisation_id' },
          { option: 'slug', path: 'slug' },
          { option: 'type', path: 'type' },
        ]),
      (client, body) => client.deployments.create(body),
    ),
  );
  const update = addWriteOutput(
    addStructuredInputOptions(
      group
        .command('update <id>')
        .description('Update a deployment registration with structured flags')
        .option('--auth-settings <json>', 'Deployment authentication settings object')
        .option('--deployment-config <json>', 'Deployment configuration object')
        .option('--is-default <boolean>', 'Make this the default deployment')
        .option('--name <name>', 'Deployment name')
        .option('--override-existing <boolean>', 'Override an existing registration')
        .option('--rotate-auth <boolean>', 'Rotate deployment authentication')
        .option('--secret-output <path>', 'Write rotated credentials to a new 0600 file')
        .option('--show-secret', 'Print rotated credentials to stdout')
        .option(
          '--status <status>',
          `Deployment status: ${knownValues(AI_GATEWAY_DEPLOYMENT_STATUSES)}`,
        )
        .option('--type <type>', `Deployment type: ${knownValues(AI_GATEWAY_DEPLOYMENT_TYPES)}`),
    ),
  );
  update.action((id, opts) =>
    runSecretWrite(
      update,
      opts,
      () =>
        buildStructuredRequest(opts, GatewayDeploymentUpdateRequestSchema, [
          { option: 'authSettings', path: 'auth_settings', parse: parseJsonOption },
          { option: 'deploymentConfig', path: 'deployment_config', parse: parseJsonOption },
          { option: 'isDefault', path: 'is_default', parse: parseBooleanOption },
          { option: 'name', path: 'name' },
          { option: 'overrideExisting', path: 'override_existing', parse: parseBooleanOption },
          { option: 'rotateAuth', path: 'rotate_auth', parse: parseBooleanOption },
          { option: 'status', path: 'status' },
          { option: 'type', path: 'type' },
        ]),
      (client, body) => client.deployments.update(id, body),
      undefined,
      {
        requiresDestination: (body) => body.rotate_auth === true,
        redactResponse: (result) =>
          redactAIGatewaySecrets('deployments.update', result, 'response'),
      },
    ),
  );
}

function registerIntegrations(root: Command): void {
  const group = showHelpOnEmpty(
    root.command('integrations').description('Manage organisation provider integrations'),
  );
  const list = addReadOutput(
    group.command('list').description('List provider integrations (admin plane)'),
  );
  list.action((opts) =>
    runList(list, opts, 'integrations', (client) => client.integrations.list()),
  );
  const get = addReadOutput(
    group.command('get <id>').description('Get a provider integration by UUID'),
  );
  get.action((id, opts) => runDetail(get, opts, (client) => client.integrations.get(id)));
  const integrationFields: NamedRequestField[] = [
    { option: 'aiProviderId', path: 'ai_provider_id' },
    { option: 'configurations', path: 'configurations', parse: parseJsonOption },
    { option: 'description', path: 'description' },
    { option: 'key', path: 'key' },
    { option: 'name', path: 'name' },
    { option: 'organisationId', path: 'organisation_id' },
    { option: 'secretMappings', path: 'secret_mappings', parse: parseJsonOption },
    { option: 'slug', path: 'slug' },
  ];
  const addIntegrationFields = (command: Command) =>
    command
      .option('--ai-provider-id <uuid>', 'Provider catalog UUID')
      .option('--configurations <json>', 'Provider configuration object')
      .option('--description <text>', 'Integration description')
      .option('--key <credential>', 'Inline provider credential (prefer secret mappings)')
      .option('--name <name>', 'Integration name')
      .option('--organisation-id <tsg>', 'Numeric TSG id')
      .option('--secret-mappings <json>', 'Secret reference mapping array')
      .option('--slug <slug>', 'Stable integration slug');
  const integrationUpdateFields = integrationFields.filter((field) =>
    ['configurations', 'description', 'key', 'name', 'secretMappings'].includes(field.option),
  );
  const addIntegrationUpdateFields = (command: Command) =>
    command
      .option('--configurations <json>', 'Provider configuration object')
      .option('--description <text>', 'Integration description')
      .option('--key <credential>', 'Inline provider credential (prefer secret mappings)')
      .option('--name <name>', 'Integration name')
      .option('--secret-mappings <json>', 'Secret reference mapping array');
  const create = addWriteOutput(
    addStructuredInputOptions(
      addIntegrationFields(
        group.command('create').description('Create an integration from structured flags'),
      ),
    ),
  );
  create.action((opts) =>
    runWrite(
      create,
      opts,
      () => buildStructuredRequest(opts, GatewayIntegrationCreateRequestSchema, integrationFields),
      (client, body) => client.integrations.create(body),
    ),
  );
  const update = addWriteOutput(
    addStructuredInputOptions(
      addIntegrationUpdateFields(
        group.command('update <id>').description('Update an integration with structured flags'),
      ),
    ),
  );
  update.action((id, opts) =>
    runWrite(
      update,
      opts,
      () =>
        buildStructuredRequest(
          opts,
          GatewayIntegrationUpdateRequestSchema,
          integrationUpdateFields,
        ),
      (client, body) => client.integrations.update(id, body),
    ),
  );
  const remove = addWriteOutput(
    group
      .command('delete <id>')
      .description('Permanently delete this integration')
      .requiredOption('--organisation-id <tsg>', 'Numeric TSG id')
      .option('--force', 'Skip confirmation prompt'),
  );
  remove.action((id, opts) =>
    runConfirmedWrite(remove, opts, `Permanently delete integration ${id}?`, (client) =>
      client.integrations.delete(id, opts.organisationId),
    ),
  );

  const models = showHelpOnEmpty(
    group.command('models').description('Inspect or replace model bindings'),
  );
  const modelsList = addReadOutput(
    models.command('list <id>').description('List models for an integration'),
  );
  modelsList.action((id, opts) =>
    runList(modelsList, opts, 'integration models', (client) => client.integrations.getModels(id)),
  );
  const modelsSet = addWriteOutput(
    addStructuredInputOptions(
      models
        .command('set <id>')
        .description('Replace model bindings')
        .option('--allow-all-models <boolean>', 'Allow every model: true or false')
        .option('--model <slug=enabled>', 'Model binding (repeatable)', collectOption)
        .option('--force', 'Skip confirmation prompt'),
    ),
  );
  modelsSet.action((id, opts) =>
    runConfirmedWrite(
      modelsSet,
      opts,
      `Replace model bindings on ${id}?`,
      () =>
        buildStructuredRequest(opts, GatewayIntegrationModelsBulkUpdateRequestSchema, [
          { option: 'allowAllModels', path: 'allow_all_models', parse: parseBooleanOption },
          { option: 'model', path: 'models', parse: parseModelBindingsOption },
        ]),
      (client, body) => client.integrations.setModels(id, body),
    ),
  );

  const workspaces = showHelpOnEmpty(
    group.command('workspaces').description('Inspect or replace workspace bindings'),
  );
  const workspacesList = addReadOutput(
    workspaces.command('list <id>').description('List workspace bindings'),
  );
  workspacesList.action((id, opts) =>
    runDetail(workspacesList, opts, (client) => client.integrations.getWorkspaces(id)),
  );
  const workspacesSet = addWriteOutput(
    addStructuredInputOptions(
      workspaces
        .command('set <id>')
        .description('Replace workspace bindings')
        .option('--create-default-provider <boolean>', 'Create defaults for new bindings')
        .option('--default-provider-slug <slug>', 'Default provider slug')
        .option('--global-access <boolean>', 'Enable or disable access to every workspace')
        .option('--preserve-existing', 'Preserve bindings not named by this command')
        .option('--workspace-binding <id=enabled>', 'Workspace binding (repeatable)', collectOption)
        .option('--force', 'Skip confirmation prompt'),
    ),
  );
  workspacesSet.action((id, opts) =>
    runConfirmedWrite(
      workspacesSet,
      opts,
      `Replace workspace bindings on ${id}?`,
      () =>
        buildStructuredRequest(
          {
            ...opts,
            overrideExistingWorkspaceAccess: !opts.preserveExisting,
          },
          GatewayIntegrationWorkspacesBulkUpdateRequestSchema,
          [
            {
              option: 'createDefaultProvider',
              path: 'create_default_provider',
              parse: parseBooleanOption,
            },
            { option: 'defaultProviderSlug', path: 'default_provider_slug' },
            {
              option: 'globalAccess',
              path: 'global_workspace_access.enabled',
              parse: parseBooleanOption,
            },
            {
              option: 'overrideExistingWorkspaceAccess',
              path: 'override_existing_workspace_access',
              parse: parseBooleanOption,
            },
            { option: 'workspaceBinding', path: 'workspaces', parse: parseBooleanBindingsOption },
          ],
        ),
      (client, body) => client.integrations.setWorkspaces(id, body),
    ),
  );
}

function registerMcp(root: Command): void {
  const mcp = showHelpOnEmpty(
    root.command('mcp').description('Manage MCP integrations and servers'),
  );
  const group = showHelpOnEmpty(
    mcp.command('integrations').description('Manage MCP server integrations'),
  );
  const list = addReadOutput(
    group.command('list').description('List MCP integrations (admin plane)'),
  );
  list.action((opts) =>
    runList(list, opts, 'MCP integrations', (client) => client.mcpIntegrations.list()),
  );
  const get = addReadOutput(
    group.command('get <id>').description('Get an MCP integration by UUID'),
  );
  get.action((id, opts) => runDetail(get, opts, (client) => client.mcpIntegrations.get(id)));
  const mcpFields: NamedRequestField[] = [
    { option: 'authType', path: 'auth_type' },
    { option: 'configurations', path: 'configurations', parse: parseJsonOption },
    { option: 'description', path: 'description' },
    { option: 'name', path: 'name' },
    { option: 'organisationId', path: 'organisation_id' },
    { option: 'secretMappings', path: 'secret_mappings', parse: parseJsonOption },
    { option: 'slug', path: 'slug' },
    { option: 'transport', path: 'transport' },
    { option: 'url', path: 'url' },
  ];
  const addMcpFields = (command: Command) =>
    command
      .option(
        '--auth-type <type>',
        `Authentication type (known: ${knownValues(AI_GATEWAY_KNOWN_MCP_AUTH_TYPES)})`,
      )
      .option('--configurations <json>', 'MCP authentication/configuration object')
      .option('--description <text>', 'Integration description')
      .option('--name <name>', 'Integration name')
      .option('--organisation-id <tsg>', 'Numeric TSG id')
      .option('--secret-mappings <json>', 'Secret reference mapping array')
      .option('--slug <slug>', 'Stable integration slug')
      .option(
        '--transport <transport>',
        `Transport (known: ${knownValues(AI_GATEWAY_KNOWN_MCP_TRANSPORTS)})`,
      )
      .option('--url <url>', 'MCP server URL');
  const mcpUpdateFields = mcpFields.filter((field) =>
    [
      'authType',
      'configurations',
      'description',
      'name',
      'secretMappings',
      'transport',
      'url',
    ].includes(field.option),
  );
  const addMcpUpdateFields = (command: Command) =>
    command
      .option(
        '--auth-type <type>',
        `Authentication type (known: ${knownValues(AI_GATEWAY_KNOWN_MCP_AUTH_TYPES)})`,
      )
      .option('--configurations <json>', 'MCP authentication/configuration object')
      .option('--description <text>', 'Integration description')
      .option('--name <name>', 'Integration name')
      .option('--secret-mappings <json>', 'Secret reference mapping array')
      .option(
        '--transport <transport>',
        `Transport (known: ${knownValues(AI_GATEWAY_KNOWN_MCP_TRANSPORTS)})`,
      )
      .option('--url <url>', 'MCP server URL');
  addCrudMutationNodes(
    group,
    'MCP integration',
    {
      createFields: mcpFields,
      createOptions: addMcpFields,
      createSchema: McpIntegrationCreateRequestSchema,
      updateFields: mcpUpdateFields,
      updateOptions: addMcpUpdateFields,
      updateSchema: McpIntegrationUpdateRequestSchema,
    },
    {
      create: (client, body) => client.mcpIntegrations.create(body),
      delete: (client, id) => client.mcpIntegrations.delete(id),
      update: (client, id, body) => client.mcpIntegrations.update(id, body),
    },
  );

  const capabilities = showHelpOnEmpty(
    group.command('capabilities').description('Inspect or replace MCP capabilities'),
  );
  const capabilitiesList = addReadOutput(
    capabilities.command('list <id>').description('List discovered MCP capabilities'),
  );
  capabilitiesList.action((id, opts) =>
    runDetail(capabilitiesList, opts, (client) => client.mcpIntegrations.getCapabilities(id)),
  );
  const capabilitiesSet = addWriteOutput(
    addStructuredInputOptions(
      capabilities
        .command('set <id>')
        .description('Replace enabled MCP capabilities')
        .option(
          '--capability <type:name=enabled>',
          `Capability binding (repeatable; type: ${knownValues(AI_GATEWAY_MUTABLE_MCP_CAPABILITY_TYPES)})`,
          collectOption,
        )
        .option('--force', 'Skip confirmation prompt'),
    ),
  );
  capabilitiesSet.action((id, opts) =>
    runConfirmedWrite(
      capabilitiesSet,
      opts,
      `Replace enabled MCP capabilities on ${id}?`,
      () =>
        buildStructuredRequest(opts, McpIntegrationCapabilitiesBulkUpdateRequestSchema, [
          { option: 'capability', path: 'capabilities', parse: parseCapabilityBindingsOption },
        ]),
      (client, body) => client.mcpIntegrations.setCapabilities(id, body),
    ),
  );

  const metadata = addReadOutput(
    group.command('metadata <id>').description('Get discovered MCP metadata'),
  );
  metadata.action((id, opts) =>
    runDetail(metadata, opts, (client) => client.mcpIntegrations.getMetadata(id)),
  );

  const workspaces = showHelpOnEmpty(
    group.command('workspaces').description('Inspect or replace MCP workspace access'),
  );
  const workspacesList = addReadOutput(
    workspaces.command('list <id>').description('Read workspace access from integration detail'),
  );
  workspacesList.action((id, opts) =>
    runDetail(workspacesList, opts, (client) => client.mcpIntegrations.get(id)),
  );
  const workspacesSet = addWriteOutput(
    addStructuredInputOptions(
      workspaces
        .command('set <id>')
        .description('Replace MCP workspace access')
        .option('--global-access <boolean>', 'Enable or disable access to every workspace')
        .option('--preserve-existing', 'Preserve bindings not named by this command')
        .option('--workspace-binding <id=enabled>', 'Workspace binding (repeatable)', collectOption)
        .option('--force', 'Skip confirmation prompt'),
    ),
  );
  workspacesSet.action((id, opts) =>
    runConfirmedWrite(
      workspacesSet,
      opts,
      `Replace MCP workspace access on ${id}?`,
      () =>
        buildStructuredRequest(
          {
            ...opts,
            overrideExistingWorkspaceAccess: !opts.preserveExisting,
          },
          McpIntegrationWorkspacesBulkUpdateRequestSchema,
          [
            {
              option: 'globalAccess',
              path: 'global_workspace_access.enabled',
              parse: parseBooleanOption,
            },
            {
              option: 'overrideExistingWorkspaceAccess',
              path: 'override_existing_workspace_access',
              parse: parseBooleanOption,
            },
            { option: 'workspaceBinding', path: 'workspaces', parse: parseBooleanBindingsOption },
          ],
        ),
      (client, body) => client.mcpIntegrations.setWorkspaces(id, body),
    ),
  );
}

function registerOrganisations(root: Command): void {
  const group = showHelpOnEmpty(
    root.command('organisations').description('Manage organisation and authentication settings'),
  );
  const self = showHelpOnEmpty(
    group.command('self').description('Manage the current organisation'),
  );
  const selfGet = addReadOutput(self.command('get').description('Get the current organisation'));
  selfGet.action((opts) => runDetail(selfGet, opts, (client) => client.organisations.getSelf()));
  const selfUpdate = addWriteOutput(
    addStructuredInputOptions(
      self
        .command('update')
        .description('Update the current organisation with structured flags')
        .option('--name <name>', 'Organisation name'),
    ),
  );
  selfUpdate.action((opts) =>
    runWrite(
      selfUpdate,
      opts,
      () =>
        buildStructuredRequest(opts, GatewayOrganisationUpdateRequestSchema, [
          { option: 'name', path: 'name' },
        ]),
      (client, body) => client.organisations.updateSelf(body),
    ),
  );
  const auth = showHelpOnEmpty(
    group.command('auth-settings').description('Manage organisation authentication settings'),
  );
  const authGet = addReadOutput(
    auth
      .command('get')
      .description('Get authentication settings')
      .requiredOption('--tsg-id <tsg>', 'Numeric TSG id')
      .option('--reveal-sensitive', 'Show SCIM and authentication secrets'),
  );
  authGet.action((opts) =>
    runDetail(authGet, opts, async (client) => {
      const result = await client.organisations.getAuthSettings(opts.tsgId);
      return opts.revealSensitive
        ? result
        : redactAIGatewaySecrets('organisations.getAuthSettings', result, 'response');
    }),
  );
  const authUpdate = addWriteOutput(
    addStructuredInputOptions(
      auth
        .command('update')
        .description('Update authentication settings with structured flags')
        .requiredOption('--tsg-id <tsg>', 'Numeric TSG id')
        .option('--auth-settings <json>', 'Authentication settings object')
        .option('--domains <domains>', 'Comma-separated allowed domains')
        .option('--scim-token <token>', 'SCIM token'),
    ),
  );
  authUpdate.action((opts) =>
    runWrite(
      authUpdate,
      opts,
      () =>
        buildStructuredRequest(opts, GatewayOrganisationAuthSettingsUpdateRequestSchema, [
          { option: 'authSettings', path: 'auth_settings', parse: parseJsonOption },
          { option: 'domains', path: 'domains', parse: parseCsvOption },
          { option: 'scimToken', path: 'scim_token' },
        ]),
      (client, body) => client.organisations.updateAuthSettings(opts.tsgId, body),
    ),
  );
}

function registerPlugins(root: Command): void {
  const group = showHelpOnEmpty(root.command('plugins').description('Manage gateway plugins'));
  const list = addReadOutput(group.command('list').description('List installed gateway plugins'));
  list.action((opts) => runList(list, opts, 'plugins', (client) => client.plugins.list()));
  const create = addWriteOutput(
    addStructuredInputOptions(
      group
        .command('create')
        .description('Install a gateway plugin from structured flags')
        .option(
          '--credential <key=value>',
          'Plugin credential (repeatable; treated as sensitive)',
          collectOption,
        )
        .option('--integration-id <uuid>', 'Integration UUID')
        .option('--organisation-id <tsg>', 'Numeric TSG id'),
    ),
  );
  create.action((opts) =>
    runWrite(
      create,
      opts,
      () =>
        buildStructuredRequest(opts, GatewayPluginCreateRequestSchema, [
          { option: 'credential', path: 'credentials', parse: parseStringMapOption },
          { option: 'integrationId', path: 'integration_id' },
          { option: 'organisationId', path: 'organisation_id' },
        ]),
      (client, body) => client.plugins.create(body),
    ),
  );
}

/** Register SDK 0.20 inventory commands, in canonical alphabetical order. */
export function registerAiGatewayInventory(root: Command): void {
  registerApiKeys(root);
  registerAuditLogs(root);
  registerConfigs(root);
  registerDeployments(root);
  const guardrails = registerScopedReads(
    root,
    'guardrails',
    'Manage workspace guardrails',
    (client) => client.guardrails,
  );
  const guardrailFields: NamedRequestField[] = [
    { option: 'actions', path: 'actions', parse: parseJsonOption },
    { option: 'checks', path: 'checks', parse: parseJsonOption },
    { option: 'name', path: 'name' },
    { option: 'workspace', path: 'workspace_id' },
  ];
  const addGuardrailFields = (command: Command) =>
    command
      .option('--actions <json>', 'Guardrail actions object')
      .option('--checks <json>', 'Guardrail checks array')
      .option('--name <name>', 'Guardrail name')
      .option('--workspace <uuid>', 'Workspace UUID');
  const guardrailUpdateFields = guardrailFields.filter((field) => field.option !== 'workspace');
  const addGuardrailUpdateFields = (command: Command) =>
    command
      .option('--actions <json>', 'Guardrail actions object')
      .option('--checks <json>', 'Guardrail checks array')
      .option('--name <name>', 'Guardrail name');
  addCrudMutationNodes(
    guardrails,
    'guardrail',
    {
      createFields: guardrailFields,
      createOptions: addGuardrailFields,
      createSchema: GatewayGuardrailCreateRequestSchema,
      updateFields: guardrailUpdateFields,
      updateOptions: addGuardrailUpdateFields,
      updateSchema: GatewayGuardrailUpdateRequestSchema,
    },
    {
      create: (client, body) => client.guardrails.create(body),
      delete: (client, id) => client.guardrails.delete(id),
      update: (client, id, body) => client.guardrails.update(id, body),
    },
  );
  registerIntegrations(root);
  registerMcp(root);
  registerOrganisations(root);
  registerPlugins(root);
  const providers = registerScopedReads(
    root,
    'providers',
    'Manage workspace provider bindings',
    (client) => client.providers,
    { sensitiveDetail: true, sensitiveOperation: 'providers.get' },
  );
  const providerFields: NamedRequestField[] = [
    { option: 'aiProviderId', path: 'ai_provider_id' },
    { option: 'expiresAt', path: 'expires_at' },
    { option: 'integrationId', path: 'integration_id' },
    { option: 'name', path: 'name' },
    { option: 'note', path: 'note' },
    { option: 'rateLimit', path: 'rate_limits', parse: parseJsonOption },
    { option: 'resetUsage', path: 'reset_usage', parse: parseBooleanOption },
    { option: 'slug', path: 'slug' },
    { option: 'usageLimit', path: 'usage_limits', parse: parseJsonOption },
    { option: 'workspace', path: 'workspace_id' },
  ];
  const addProviderFields = (command: Command) =>
    command
      .option('--ai-provider-id <uuid>', 'Provider catalog UUID')
      .option('--expires-at <iso>', 'Expiration as ISO-8601')
      .option('--integration-id <uuid>', 'Organisation integration UUID')
      .option('--name <name>', 'Provider binding name')
      .option('--note <text>', 'Operator note')
      .option('--rate-limit <json>', 'Rate-limit object')
      .option('--reset-usage <boolean>', 'Reset accumulated usage: true or false')
      .option('--slug <slug>', 'Provider binding slug')
      .option('--usage-limit <json>', 'Usage-limit object')
      .option('--workspace <uuid>', 'Workspace UUID');
  const providerUpdateFields = providerFields.filter((field) =>
    ['expiresAt', 'name', 'note', 'rateLimit', 'resetUsage', 'usageLimit'].includes(field.option),
  );
  const addProviderUpdateFields = (command: Command) =>
    command
      .option('--expires-at <iso>', 'Expiration as ISO-8601')
      .option('--name <name>', 'Provider binding name')
      .option('--note <text>', 'Operator note')
      .option('--rate-limit <json>', 'Rate-limit object')
      .option('--reset-usage <boolean>', 'Reset accumulated usage: true or false')
      .option('--usage-limit <json>', 'Usage-limit object');
  addCrudMutationNodes(
    providers,
    'provider',
    {
      createFields: providerFields,
      createOptions: addProviderFields,
      createSchema: GatewayProviderCreateRequestSchema,
      updateFields: providerUpdateFields,
      updateOptions: addProviderUpdateFields,
      updateSchema: GatewayProviderUpdateRequestSchema,
    },
    {
      create: (client, body) => client.providers.create(body),
      delete: (client, id) => client.providers.delete(id),
      update: (client, id, body) => client.providers.update(id, body),
    },
  );
}
