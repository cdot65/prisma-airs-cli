import { mkdtemp, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { readTenantStore, type TenantEntry } from '../../src/config/tenants.js';

/** The operator's selected tenant for live e2e runs; AIRS_E2E_TENANT picks another registered one. */
export function selectedTenant(name = process.env.AIRS_E2E_TENANT): TenantEntry {
  const store = readTenantStore();
  const wanted = name ?? store.active;
  const entry = store.tenants.find((tenant) => tenant.name === wanted);
  if (!entry)
    throw new Error(
      "No tenant selected. Run 'airs tenant switch <name>' or set AIRS_E2E_TENANT to a registered tenant",
    );
  return entry;
}

/** Private registry selecting `tenant`; pass as PRISMA_AIRS_TENANTS_PATH to the spawned CLI. */
export async function isolatedRegistry(tenant: TenantEntry = selectedTenant()): Promise<string> {
  const directory = await mkdtemp(join(tmpdir(), 'airs-e2e-registry-'));
  const path = join(directory, 'tenants.json');
  await writeFile(
    path,
    `${JSON.stringify({ version: 1, active: tenant.name, tenants: [tenant] }, null, 2)}\n`,
    { mode: 0o600 },
  );
  return path;
}
