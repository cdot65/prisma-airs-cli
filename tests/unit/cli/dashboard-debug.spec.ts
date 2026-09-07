import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { expect, it, vi } from 'vitest';
import { installDebugLogger } from '../../../src/cli/debug-logger.js';

it.each([
  'dashboard/v2/sessions/sessiontransaction',
  'reports/scancontent',
  'dashboard/v2/apps/appslist',
])('logs SCM %s metadata without cloning or persisting bodies', async (path) => {
  const directory = mkdtempSync(join(tmpdir(), 'airs-dashboard-debug-'));
  const original = globalThis.fetch;
  const response = Response.json({ prompt: 'PRIVATE-TEXT', attributes: { user_ip: 'PRIVATE-IP' } });
  const clone = vi.spyOn(response, 'clone');
  globalThis.fetch = vi.fn().mockResolvedValue(response);
  const log = join(directory, 'debug.jsonl');
  const logger = installDebugLogger(log);
  try {
    await fetch(`https://api.apps.paloaltonetworks.com/aisec/v1/mgmt/${path}`, {
      headers: { authorization: 'Bearer PRIVATE-TOKEN' },
    });
    const output = readFileSync(log, 'utf8');
    expect(output).not.toContain('PRIVATE');
    expect(output).toContain('BODY OMITTED');
    expect(clone).not.toHaveBeenCalled();
    expect(JSON.parse(output).response.status).toBe(200);
  } finally {
    logger.teardown();
    globalThis.fetch = original;
    rmSync(directory, { recursive: true, force: true });
  }
});
