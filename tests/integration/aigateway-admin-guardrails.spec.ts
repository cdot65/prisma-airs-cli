import { execFile } from 'node:child_process';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { createServer, type Server } from 'node:http';
import type { AddressInfo } from 'node:net';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { promisify } from 'node:util';

const exec = promisify(execFile);
let directory: string;
let server: Server;
let env: NodeJS.ProcessEnv;
let requests: string[];
let deny = false;
async function cli(...args: string[]) {
  const entry = process.env.AIRS_CLI_ENTRY
    ? [resolve(process.env.AIRS_CLI_ENTRY)]
    : ['--import', resolve('node_modules/tsx/dist/loader.mjs'), resolve('src/cli/index.ts')];
  try {
    return {
      ...(await exec(
        process.execPath,
        [...entry, 'aigateway', 'admin-guardrails', ...args, '--output', 'json'],
        { cwd: directory, env, timeout: 20000 },
      )),
      code: 0,
    };
  } catch (error) {
    return error as { stdout: string; stderr: string; code: number };
  }
}
beforeEach(async () => {
  requests = [];
  deny = false;
  directory = await mkdtemp(join(tmpdir(), 'airs-admin-wire-'));
  server = createServer(async (req, res) => {
    requests.push(`${req.method} ${req.url}`);
    res.setHeader('Content-Type', 'application/json');
    if (req.url === '/token') {
      res.end(
        JSON.stringify({ access_token: 'mock-token', token_type: 'Bearer', expires_in: 3600 }),
      );
      return;
    }
    if (req.headers.authorization !== 'Bearer mock-token' || req.headers['x-tsg-id'] !== '123') {
      res.writeHead(401).end('{}');
      return;
    }
    if (req.url?.startsWith('/admin/v2/guardrails')) {
      if (deny) {
        res.writeHead(403).end('{"message":"Denied"}');
        return;
      }
      res.end(JSON.stringify({ data: [], total: 0 }));
      return;
    }
    res.writeHead(404).end('{}');
  });
  await new Promise<void>((done) => server.listen(0, '127.0.0.1', done));
  const base = `http://127.0.0.1:${(server.address() as AddressInfo).port}`;
  const configPath = join(directory, 'tenant.json');
  await writeFile(
    configPath,
    JSON.stringify({
      mgmtClientId: 'fixture',
      mgmtClientSecret: 'FAKE-SECRET',
      mgmtTsgId: '123',
      mgmtTokenEndpoint: `${base}/token`,
      aiGwDataEndpoint: `${base}/control/v2`,
      aiGwAdminEndpoint: `${base}/admin/v2`,
    }),
    { mode: 0o600 },
  );
  const registry = join(directory, 'tenants.json');
  await writeFile(
    registry,
    JSON.stringify({
      version: 1,
      active: 'fixture',
      tenants: [{ name: 'fixture', tsgId: '123', configPath }],
    }),
    { mode: 0o600 },
  );
  env = { ...process.env, PRISMA_AIRS_TENANTS_PATH: registry, NO_COLOR: '1' };
});
afterEach(async () => {
  await new Promise<void>((done) => server.close(() => done()));
  await rm(directory, { recursive: true, force: true });
});
it('routes the real CLI and SDK to the selected admin endpoint with OAuth and TSG', async () => {
  const result = await cli('list', '--page-size', '100', '--current-page', '0');
  expect(result.code, result.stderr).toBe(0);
  expect(JSON.parse(result.stdout)).toEqual([]);
  expect(requests).toEqual([
    'POST /token',
    'GET /admin/v2/guardrails?page_size=100&current_page=0',
  ]);
  expect(result.stdout + result.stderr).not.toMatch(/mock-token|FAKE-SECRET/);
});
it('refuses invalid pagination before OAuth and does not retry denied routes on the control plane', async () => {
  const invalid = await cli('list', '--page-size', '1001');
  expect(invalid.code).toBe(2);
  expect(requests).toEqual([]);
  deny = true;
  const rejected = await cli('list');
  expect(rejected.code).toBe(1);
  // The existing OAuth adapter permits one refresh on denial; both attempts stay on admin.
  expect(requests).toEqual([
    'POST /token',
    'GET /admin/v2/guardrails',
    'POST /token',
    'GET /admin/v2/guardrails',
  ]);
  expect(rejected.stdout + rejected.stderr).not.toMatch(/mock-token|FAKE-SECRET/);
});
