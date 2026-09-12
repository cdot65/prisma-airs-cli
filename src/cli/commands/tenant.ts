import type { Command } from 'commander';
import {
  createManagedTenant,
  isTenantSecret,
  setTenantSetting,
  unsetTenantSetting,
  validateTenantSettingKey,
} from '../../config/tenant-settings.js';
import {
  createTenant,
  deleteTenant,
  readTenantConfig,
  readTenantStore,
  switchTenant,
  type TenantEntry,
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

const KEY_VALUE_COLUMNS = [
  { key: 'key', label: 'Key' },
  { key: 'value', label: 'Value' },
];

/** Registered entry by name, or the selected tenant when no name is given. */
function resolveEntry(name: string | undefined): TenantEntry {
  const store = readTenantStore();
  const selected = name ?? store.active;
  if (!selected)
    throw new Error(
      store.tenants.length
        ? `No tenant selected; pass a name or run 'airs tenant switch <name>' (registered: ${store.tenants.map((entry) => entry.name).join(', ')})`
        : "No tenant selected; run 'airs tenant create <name>' first",
    );
  const entry = store.tenants.find((value) => value.name === selected);
  if (!entry) throw new Error('Tenant not found');
  return entry;
}

function redact(key: string, value: unknown): unknown {
  return isTenantSecret(key) && value ? '[REDACTED]' : (value ?? '');
}

export function registerTenantCommand(program: Command): void {
  const tenant = program
    .command('tenant')
    .description('Create, configure and select tenants (the only configuration source)')
    .addHelpText(
      'after',
      examples(
        'airs tenant create development',
        'airs tenant switch development',
        'airs tenant set development defaultOutput yaml',
        'airs tenant set development airsApiKey',
        'airs tenant unset development defaultOutput',
        'airs tenant get development mgmtTsgId',
        'airs tenant create production --config /secure/production.json',
        'airs tenant read',
        'airs tenant path',
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
    .description('Update one tenant setting; prompt when omitted, hide secrets')
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
    .command('unset <name> <key>')
    .description('Remove one tenant setting so the default applies; credentials cannot be cleared')
    .action(async (name: string, key: string) => {
      try {
        const removed = await unsetTenantSetting(name, key);
        if (removed) ui.success(`Removed ${key} from tenant ${name}; selection unchanged.`);
        else ui.info(`${key} is not set for tenant ${name} — nothing to do`);
      } catch (error) {
        fail(error);
      }
    });

  const get = tenant
    .command('get <name> <key>')
    .description('Print one effective tenant setting; credential values are redacted')
    .option('--output <format>', 'Output format: pretty, table, markdown, csv, json, yaml')
    .action(async (name: string, key: string, opts: { output?: string }) => {
      try {
        validateTenantSettingKey(key);
        const format = await resolveOutput(get, opts, { ignoreConfig: true });
        const entry = resolveEntry(name);
        const config = readTenantConfig(entry.configPath, entry.tsgId) as Record<string, unknown>;
        const value = redact(key, config[key]);
        if (format === 'pretty') console.log(String(value));
        else console.log(formatOutput([{ key, value }], KEY_VALUE_COLUMNS, format));
      } catch (error) {
        fail(error);
      }
    });

  tenant
    .command('switch <name>')
    .description('Persist the selected tenant for subsequent commands')
    .action(async (name: string) => {
      try {
        const entry = await switchTenant(name);
        ui.success(`Selected ${entry.name} (TSG ${entry.tsgId})`);
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
        if (store.tenants.length === 0 && format === 'pretty') {
          ui.emptyList('tenants');
          ui.status("Run 'airs tenant create <name>' to register one.");
          return;
        }
        if (store.active === null && store.tenants.length)
          ui.status("No tenant selected; run 'airs tenant switch <name>'.");
        const rows = store.tenants.map((entry) => ({
          ...entry,
          active: entry.name === store.active,
        }));
        console.log(formatOutput(rows, COLUMNS, format === 'pretty' ? 'table' : format));
      } catch (error) {
        fail(error);
      }
    });

  const read = tenant
    .command('read [name]')
    .description(
      'Read a tenant config with all credential values redacted; defaults to the selected tenant',
    )
    .option('--output <format>', 'Output format: pretty, table, markdown, csv, json, yaml')
    .action(async (name: string | undefined, opts: { output?: string }) => {
      try {
        const format = await resolveOutput(read, opts, { ignoreConfig: true });
        const entry = resolveEntry(name);
        const config = readTenantConfig(entry.configPath, entry.tsgId);
        const rows = Object.entries(config).map(([key, value]) => ({
          key,
          value: redact(key, value),
        }));
        ui.status(`Tenant ${entry.name} (TSG ${entry.tsgId}): ${entry.configPath}`);
        console.log(formatOutput(rows, KEY_VALUE_COLUMNS, format === 'pretty' ? 'table' : format));
      } catch (error) {
        fail(error);
      }
    });

  tenant
    .command('path [name]')
    .description('Print the config file path of a tenant; defaults to the selected tenant')
    .action((name: string | undefined) => {
      try {
        console.log(resolveEntry(name).configPath);
      } catch (error) {
        fail(error);
      }
    });

  tenant
    .command('delete <name>')
    .description(
      'Unregister a tenant; retain its source config file and clear the selection if it was selected',
    )
    .option('--force', 'Skip interactive confirmation')
    .action(async (name: string, opts: { force?: boolean }) => {
      try {
        const store = readTenantStore();
        if (!store.tenants.some((entry) => entry.name === name))
          throw new Error('Tenant not found');
        const selected = store.active === name;
        await confirmOrAbort(
          `Unregister tenant ${name}?${selected ? ' It is the selected tenant; no tenant will be selected afterwards.' : ''} Its config file will be retained.`,
          Boolean(opts.force),
          { action: `unregister tenant ${name}` },
        );
        const { selectionCleared } = await deleteTenant(name);
        ui.success(
          `Unregistered ${name}; source config file retained${selectionCleared ? '; no tenant is selected' : ''}`,
        );
      } catch (error) {
        fail(error);
      }
    });
}
