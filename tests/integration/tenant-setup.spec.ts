import { execFile } from 'node:child_process';
import { mkdtemp, readdir, readFile, rm } from 'node:fs/promises';
import { createServer } from 'node:http';
import type { AddressInfo } from 'node:net';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

let directory: string;
let env: NodeJS.ProcessEnv;
beforeEach(async () => {
  directory = await mkdtemp(join(tmpdir(), 'airs-tenant-setup-cli-'));
  env = Object.fromEntries(
    Object.entries(process.env).filter(
      ([key]) => !key.startsWith('PANW_') && !key.startsWith('PRISMA_AIRS_'),
    ),
  );
  env.PRISMA_AIRS_TENANTS_PATH = join(directory, 'tenants.json');
  env.NO_COLOR = '1';
});
afterEach(async () => {
  await rm(directory, { recursive: true, force: true });
});

function cli(
  args: string[],
  stdin?: string,
  code = 0,
  group = 'tenant',
): Promise<{ stdout: string; stderr: string }> {
  const entry = process.env.AIRS_CLI_ENTRY
    ? [resolve(process.env.AIRS_CLI_ENTRY)]
    : ['--import', resolve('node_modules/tsx/dist/loader.mjs'), resolve('src/cli/index.ts')];
  return new Promise((done, reject) => {
    const child = execFile(
      process.execPath,
      [...entry, group, ...args],
      { cwd: directory, env, timeout: 20000 },
      (error, stdout, stderr) => {
        try {
          expect(error?.code ?? 0, stderr).toBe(code);
          expect(stdout + stderr).not.toContain('FAKE-SECRET');
          done({ stdout, stderr });
        } catch (error) {
          reject(error);
        }
      },
    );
    child.stdin?.end(stdin);
  });
}

it('uses individually configured tenant credentials for a real CLI OAuth and profile-list workflow against a local API', async () => {
  const requests: string[] = [];
  const server = createServer(async (req, res) => {
    res.setHeader('Content-Type', 'application/json');
    const chunks: Buffer[] = [];
    for await (const chunk of req) chunks.push(Buffer.from(chunk));
    if (req.url === '/oauth/token') {
      const form = new URLSearchParams(Buffer.concat(chunks).toString());
      const basic = Buffer.from(
        (req.headers.authorization ?? '').replace('Basic ', ''),
        'base64',
      ).toString();
      if (
        form.get('scope') !== 'tsg_id:100' ||
        (basic !== 'client-100:FAKE-SECRET' &&
          (form.get('client_id') !== 'client-100' || form.get('client_secret') !== 'FAKE-SECRET'))
      ) {
        res.writeHead(401).end('{}');
        return;
      }
      requests.push('oauth:100');
      res.end(
        JSON.stringify({
          access_token: 'local-test-token',
          token_type: 'Bearer',
          expires_in: 3600,
        }),
      );
    } else if (
      req.url?.includes('/profiles/tsg/100') &&
      req.headers.authorization === 'Bearer local-test-token'
    ) {
      requests.push('profiles:100');
      res.end(JSON.stringify({ ai_profiles: [], next_offset: 0 }));
    } else res.writeHead(404).end('{}');
  });
  await new Promise<void>((done) => server.listen(0, '127.0.0.1', done));
  const base = `http://127.0.0.1:${(server.address() as AddressInfo).port}`;
  try {
    await cli(
      ['create', 'dev', '--tsg-id', '100', '--client-id', 'client-100', '--client-secret-stdin'],
      'FAKE-SECRET',
    );
    await cli(['set', 'dev', 'mgmtTokenEndpoint', `${base}/oauth/token`]);
    await cli(['set', 'dev', 'mgmtEndpoint', base]);
    await cli(['switch', 'dev']);
    expect(
      JSON.parse(
        (await cli(['profiles', 'list', '--output', 'json'], undefined, 0, 'runtime')).stdout,
      ),
    ).toEqual([]);
    expect(requests).toEqual(['oauth:100', 'profiles:100']);
  } finally {
    server.closeAllConnections();
    await new Promise<void>((done, reject) =>
      server.close((error) => (error ? reject(error) : done())),
    );
  }
}, 60000);

it('creates without JSON, configures one field at a time, rotates secrets and switches', async () => {
  await cli(
    ['create', 'dev', '--tsg-id', '100', '--client-id', 'client-100', '--client-secret-stdin'],
    'FAKE-SECRET\n',
  );
  await cli(['set', 'dev', 'defaultOutput', 'yaml']);
  await cli(['set', 'dev', 'scanConcurrency', '3']);
  await cli(['set', 'dev', 'mgmtClientSecret', '--stdin'], 'FAKE-SECRET-ROTATED\n');
  await cli(['set', 'dev', 'mgmtTokenEndpoint', 'https://example.test/token']);
  await cli(['switch', 'dev']);
  const output = JSON.parse((await cli(['read', 'dev', '--output', 'json'])).stdout);
  expect(output).toContainEqual({ key: 'defaultOutput', value: 'yaml' });
  expect(output).toContainEqual({ key: 'mgmtClientSecret', value: '[REDACTED]' });
  const registry = JSON.parse(await readFile(join(directory, 'tenants.json'), 'utf8'));
  expect(registry.active).toBe('dev');
  expect(JSON.parse(await readFile(registry.tenants[0].configPath, 'utf8'))).toEqual({
    mgmtTsgId: '100',
    mgmtClientId: 'client-100',
    mgmtClientSecret: 'FAKE-SECRET-ROTATED',
    defaultOutput: 'yaml',
    scanConcurrency: 3,
    mgmtTokenEndpoint: 'https://example.test/token',
  });
}, 60000);

it('fails clearly without a terminal or stdin instead of creating an incomplete tenant', async () => {
  expect((await cli(['create', 'dev'], undefined, 1)).stderr).toContain('requires a terminal');
  await cli(
    ['create', 'dev', '--tsg-id', '100', '--client-id', 'client-100', '--client-secret-stdin'],
    '',
    1,
  );
  await cli(
    ['create', 'dev', '--tsg-id', '100', '--client-id', 'client-100', '--client-secret-stdin'],
    'FAKE-SECRET\nsecond-line',
    1,
  );
  expect(await readdir(directory)).toEqual([]);
}, 60000);

it('refuses mixed input modes, secret arguments and identity changes', async () => {
  await cli(['create', 'dev', '--config', '/unused', '--tsg-id', '100'], undefined, 1);
  await cli(
    ['create', 'dev', '--tsg-id', '100', '--client-id', 'client-100', '--client-secret-stdin'],
    'FAKE-SECRET',
  );
  await cli(['set', 'dev', 'mgmtClientSecret', 'FAKE-SECRET-ARGV'], undefined, 1);
  await cli(['set', 'dev', 'defaultOutput', 'json', '--stdin'], 'yaml', 1);
  await cli(['set', 'dev', 'mgmtTsgId', '200'], undefined, 1);
  await cli(['set', 'dev', 'notAKey', 'FAKE-SECRET'], undefined, 1);
  expect(JSON.parse((await cli(['list', '--output', 'json'])).stdout)[0].tsgId).toBe('100');
}, 60000);
