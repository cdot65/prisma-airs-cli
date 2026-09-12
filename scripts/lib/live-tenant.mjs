// Resolve the operator's selected tenant for live end-to-end scripts. The CLI reads
// configuration only from tenant files, so scripts locate the selected tenant through
// the registry instead of an environment variable, then hand the spawned CLI an
// isolated registry that selects the same file.

import { readFileSync } from 'node:fs';
import { mkdtemp, writeFile } from 'node:fs/promises';
import { homedir, tmpdir } from 'node:os';
import { isAbsolute, join } from 'node:path';

export function registryPath() {
  if (process.env.PRISMA_AIRS_TENANTS_PATH) return process.env.PRISMA_AIRS_TENANTS_PATH;
  const state = process.env.XDG_STATE_HOME;
  if (state && !isAbsolute(state)) throw new Error('XDG_STATE_HOME must be an absolute path');
  return join(state || join(homedir(), '.local', 'state'), 'prisma-airs', 'tenants.json');
}

/** Registered tenants and the selected name, or an empty registry when none exists. */
export function readRegistry() {
  try {
    return JSON.parse(readFileSync(registryPath(), 'utf8'));
  } catch (error) {
    if (error?.code === 'ENOENT') return { version: 1, active: null, tenants: [] };
    throw error;
  }
}

/** The selected tenant entry ({ name, configPath, tsgId }); throws with guidance when none is selected. */
export function selectedTenant(name = process.env.AIRS_E2E_TENANT) {
  const registry = readRegistry();
  const wanted = name ?? registry.active;
  const entry = registry.tenants.find((tenant) => tenant.name === wanted);
  if (!entry)
    throw new Error(
      "No tenant selected. Run 'airs tenant switch <name>' or set AIRS_E2E_TENANT to a registered tenant",
    );
  return entry;
}

/** Write a private registry that selects `tenant` and return its path for PRISMA_AIRS_TENANTS_PATH. */
export async function isolatedRegistry(tenant = selectedTenant(), directory) {
  const dir = directory ?? (await mkdtemp(join(tmpdir(), 'airs-e2e-registry-')));
  const path = join(dir, 'tenants.json');
  await writeFile(
    path,
    `${JSON.stringify({ version: 1, active: tenant.name, tenants: [tenant] }, null, 2)}\n`,
    { mode: 0o600 },
  );
  return path;
}
