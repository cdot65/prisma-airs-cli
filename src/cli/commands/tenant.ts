import type { Command } from 'commander';
import {
  createManagedTenant,
  isTenantSecret,
  setTenantSetting,
  validateTenantSettingKey,
} from '../../config/tenant-settings.js';
import {
  createTenant,
  defaultTenantConfigPath,
  deleteTenant,
  expandConfigPath,
  readTenantConfig,
  readTenantStore,
  switchTenant,
  validateTenantName,
} from '../../config/tenants.js';
import { confirmOrAbort } from '../confirm.js';
import { examples } from '../examples.js';
import { fail, formatOutput, resolveOutput, ui } from '../renderer/index.js';
import { promptTenantValue, readTenantStdin } from '../tenant-input.js';

function tenantInputFailure(error: unknown): void {
  if (error instanceof Error && error.name === 'ExitPromptError') {
    ui.status('Cancelled; no configuration saved.');
    process.exitCode = 130;
    return;
  }
  fail(error);
}

const COLUMNS = [
  { key: 'name', label: 'Tenant' },
  { key: 'active', label: 'Selected' },
  { key: 'tsgId', label: 'TSG ID' },
  { key: 'configPath', label: 'Config file' },
];

export function registerTenantCommand(program: Command): void {
  const tenant = program
    .command('tenant')
    .description('Create, configure and select named tenants')
    .addHelpText(
      'after',
      examples(
        'airs tenant create development',
        'airs tenant set development defaultOutput yaml',
        'airs tenant create production --config /secure/production.json',
        'airs tenant switch production',
        'airs tenant read',
        'airs tenant switch default',
      ),
    );

  tenant
    .command('create <name>')
    .description(
      'Create a config with guided prompts, or register an existing JSON file; does not activate it',
    )
    .option('--config <path>', 'Existing Prisma AIRS JSON config file (no copying or editing)')
    .option('--tsg-id <id>', 'Tenant service group ID for a new config')
    .option('--client-id <id>', 'OAuth client ID for a new config')
    .option('--client-secret-stdin', 'Read the new config OAuth secret from piped stdin')
    .action(
      async (
        name: string,
        opts: { config?: string; tsgId?: string; clientId?: string; clientSecretStdin?: boolean },
      ) => {
        try {
          validateTenantName(name);
          if (
            opts.config &&
            (opts.tsgId !== undefined || opts.clientId !== undefined || opts.clientSecretStdin)
          )
            throw new Error('--config cannot be combined with new-config options');
          if (!opts.config && readTenantStore().tenants.some((entry) => entry.name === name))
            throw new Error('Tenant name already exists');
          const entry = opts.config
            ? await createTenant(name, opts.config)
            : await createManagedTenant(name, {
                mgmtTsgId:
                  opts.tsgId ?? (await promptTenantValue('Tenant service group ID (mgmtTsgId):')),
                mgmtClientId:
                  opts.clientId ?? (await promptTenantValue('OAuth client ID (mgmtClientId):')),
                mgmtClientSecret: opts.clientSecretStdin
                  ? await readTenantStdin()
                  : await promptTenantValue('OAuth client secret (mgmtClientSecret):', true),
              });
          ui.success(
            `Registered ${entry.name} (TSG ${entry.tsgId}); ${opts.config ? 'config file unchanged' : 'private config created'}. Use airs tenant switch ${entry.name}.`,
          );
        } catch (error) {
          tenantInputFailure(error);
        }
      },
    );

  tenant
    .command('set <name> <key> [value]')
    .description('Update one named tenant setting; prompt when omitted, hide secrets')
    .option('--stdin', 'Read one value from piped stdin (recommended for automated secret updates)')
    .action(
      async (name: string, key: string, value: string | undefined, opts: { stdin?: boolean }) => {
        try {
          validateTenantSettingKey(key);
          if (!readTenantStore().tenants.some((entry) => entry.name === name))
            throw new Error('Tenant not found; register a named tenant first');
          if (opts.stdin && value !== undefined)
            throw new Error('Use a value or --stdin, not both');
          if (isTenantSecret(key) && value !== undefined)
            throw new Error(
              'Do not pass credentials as arguments. Omit the value for a hidden prompt, or use --stdin.',
            );
          const setting = opts.stdin
            ? await readTenantStdin()
            : (value ?? (await promptTenantValue(`${key}:`, isTenantSecret(key))));
          await setTenantSetting(name, key, setting);
          ui.success(`Updated ${key} for tenant ${name}; selection unchanged.`);
        } catch (error) {
          tenantInputFailure(error);
        }
      },
    );

  tenant
    .command('switch <name>')
    .description(
      'Persist the selected tenant; default restores legacy config/environment resolution',
    )
    .action(async (name: string) => {
      try {
        const entry = await switchTenant(name);
        ui.success(
          entry
            ? `Selected ${entry.name} (TSG ${entry.tsgId})`
            : 'Selected default; explicit config/environment overrides still apply',
        );
      } catch (error) {
        fail(error);
      }
    });

  const list = tenant
    .command('list')
    .description('List registrations without reading credentials')
    .option('--output <format>', 'Output format: pretty, table, markdown, csv, json, yaml')
    .action(async (opts: { output?: string }) => {
      try {
        const format = await resolveOutput(list, opts, { ignoreConfig: true });
        const store = readTenantStore();
        const explicit = process.env.PRISMA_AIRS_CONFIG_PATH;
        const rows = [
          {
            name: 'default',
            active: store.active === null,
            tsgId: '',
            configPath: defaultTenantConfigPath(),
          },
          ...store.tenants.map((entry) => ({ ...entry, active: entry.name === store.active })),
        ];
        if (explicit)
          ui.status('PRISMA_AIRS_CONFIG_PATH overrides the selected tenant for API commands.');
        console.log(formatOutput(rows, COLUMNS, format === 'pretty' ? 'table' : format));
      } catch (error) {
        fail(error);
      }
    });

  const read = tenant
    .command('read [name]')
    .description('Read a tenant config with all credential values redacted')
    .option('--output <format>', 'Output format: pretty, table, markdown, csv, json, yaml')
    .action(async (name: string | undefined, opts: { output?: string }) => {
      try {
        const format = await resolveOutput(read, opts, { ignoreConfig: true });
        const store = readTenantStore();
        const selected = name ?? store.active ?? 'default';
        const entry = store.tenants.find((value) => value.name === selected);
        if (selected !== 'default' && !entry) throw new Error('Tenant not found');
        const path =
          entry?.configPath ??
          expandConfigPath(process.env.PRISMA_AIRS_CONFIG_PATH || defaultTenantConfigPath());
        const config = readTenantConfig(path, entry?.tsgId);
        const rows = Object.entries(config).map(([key, value]) => ({
          key,
          value: /key|secret|token|password/i.test(key) && value ? '[REDACTED]' : (value ?? ''),
        }));
        ui.status(`Tenant ${selected}${entry ? ` (TSG ${entry.tsgId})` : ''}: ${path}`);
        console.log(
          formatOutput(
            rows,
            [
              { key: 'key', label: 'Key' },
              { key: 'value', label: 'Value' },
            ],
            format === 'pretty' ? 'table' : format,
          ),
        );
      } catch (error) {
        fail(error);
      }
    });

  tenant
    .command('delete <name>')
    .description('Unregister an inactive tenant; retain its source config file')
    .option('--force', 'Skip interactive confirmation; active tenant deletion remains refused')
    .action(async (name: string, opts: { force?: boolean }) => {
      try {
        const store = readTenantStore();
        if (name === 'default' || name === store.active)
          throw new Error('Switch away first; default and active tenants cannot be deleted');
        if (!store.tenants.some((entry) => entry.name === name))
          throw new Error('Tenant not found');
        await confirmOrAbort(
          `Unregister tenant ${name}? Its config file will be retained.`,
          Boolean(opts.force),
          { action: `unregister tenant ${name}` },
        );
        await deleteTenant(name);
        ui.success(`Unregistered ${name}; source config file retained`);
      } catch (error) {
        fail(error);
      }
    });
}
