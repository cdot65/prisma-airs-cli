import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterAll } from 'vitest';

// Unit/integration tests must never depend on or mutate the user's selected tenant.
// Each test file receives its own empty registry; tests may explicitly override it.
const directory = mkdtempSync(join(tmpdir(), 'airs-test-tenant-state-'));
const previous = process.env.PRISMA_AIRS_TENANTS_PATH;
process.env.PRISMA_AIRS_TENANTS_PATH = join(directory, 'tenants.json');
afterAll(() => {
  if (previous === undefined) delete process.env.PRISMA_AIRS_TENANTS_PATH;
  else process.env.PRISMA_AIRS_TENANTS_PATH = previous;
  rmSync(directory, { recursive: true, force: true });
});
