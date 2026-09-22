import { mkdir, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';

export const TEST_CREDENTIALS = {
  mgmtClientId: 'client',
  mgmtClientSecret: 'secret',
  mgmtTsgId: '123',
} as const;

export interface TestTenantSpec {
  name: string;
  config?: Record<string, unknown>;
}

/**
 * Write tenant config files plus a registry selecting `active`, bypassing the CLI so
 * tests can shape files freely. Every config gets the shared test credentials unless
 * overridden; the registry pins each tenant to its file's `mgmtTsgId`.
 */
export async function writeTestRegistry(
  registryPath: string,
  tenants: TestTenantSpec[],
  active: string | null,
): Promise<Record<string, string>> {
  const directory = join(dirname(registryPath), 'configs');
  await mkdir(directory, { recursive: true, mode: 0o700 });
  const paths: Record<string, string> = {};
  const entries = [];
  for (const spec of tenants) {
    const merged = { ...TEST_CREDENTIALS, ...spec.config };
    const configPath = join(directory, `${spec.name}.json`);
    await writeFile(configPath, `${JSON.stringify(merged, null, 2)}\n`, { mode: 0o600 });
    paths[spec.name] = configPath;
    entries.push({ name: spec.name, configPath, tsgId: String(merged.mgmtTsgId) });
  }
  await mkdir(dirname(registryPath), { recursive: true, mode: 0o700 });
  await writeFile(
    registryPath,
    `${JSON.stringify({ version: 1, active, tenants: entries }, null, 2)}\n`,
    { mode: 0o600 },
  );
  return paths;
}

/**
 * Register and select one tenant in the registry that `tests/setup.ts` isolated for this
 * file (or an explicit `registryPath`). Returns the config file path.
 */
export async function useTestTenant(
  config: Record<string, unknown> = {},
  options: { name?: string; registryPath?: string } = {},
): Promise<{ name: string; configPath: string; registryPath: string }> {
  const registryPath = options.registryPath ?? process.env.PRISMA_AIRS_TENANTS_PATH;
  if (!registryPath) throw new Error('PRISMA_AIRS_TENANTS_PATH is not set for this test');
  const name = options.name ?? 'test';
  const paths = await writeTestRegistry(registryPath, [{ name, config }], name);
  return { name, configPath: paths[name], registryPath };
}
