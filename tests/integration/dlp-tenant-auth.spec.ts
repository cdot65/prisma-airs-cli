import { execFile } from 'node:child_process';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { createServer, type Server } from 'node:http';
import type { AddressInfo } from 'node:net';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { promisify } from 'node:util';

const exec = promisify(execFile);
let directory: string;
let server: Server;
let env: NodeJS.ProcessEnv;
let requests: Array<{ method: string; path: string; tsg: string }>;
let configs: string[];
const resources = new Map<string, Record<string, unknown>>();

async function cli(args: string[], extraEnv: NodeJS.ProcessEnv = {}, code = 0) {
  const entry = process.env.AIRS_CLI_ENTRY
    ? [resolve(process.env.AIRS_CLI_ENTRY)]
    : ['--import', resolve('node_modules/tsx/dist/loader.mjs'), resolve('src/cli/index.ts')];
  let result: { stdout: string; stderr: string; code?: number };
  try {
    result = await exec(process.execPath, [...entry, ...args], {
      cwd: directory,
      env: { ...env, ...extraEnv },
      timeout: 20000,
    });
  } catch (error) {
    result = error as typeof result;
  }
  expect(result.code ?? 0, result.stderr).toBe(code);
  expect(result.stdout + result.stderr).not.toMatch(/FAKE-SECRET|mock-token/);
  return result;
}

beforeEach(async () => {
  directory = await mkdtemp(join(tmpdir(), 'airs-dlp-auth-'));
  requests = [];
  resources.clear();
  server = createServer(async (req, res) => {
    const chunks: Buffer[] = [];
    for await (const chunk of req) chunks.push(Buffer.from(chunk));
    const body = Buffer.concat(chunks).toString();
    const path = new URL(req.url ?? '/', 'http://localhost').pathname;
    res.setHeader('Content-Type', 'application/json');
    if (path === '/oauth/token') {
      const form = new URLSearchParams(body);
      const tsg = form.get('scope')?.replace('tsg_id:', '') ?? '';
      const basic = Buffer.from(
        (req.headers.authorization ?? '').replace('Basic ', ''),
        'base64',
      ).toString();
      if (
        !['100', '200'].includes(tsg) ||
        (basic !== `client-${tsg}:FAKE-SECRET-${tsg}` &&
          (form.get('client_id') !== `client-${tsg}` ||
            form.get('client_secret') !== `FAKE-SECRET-${tsg}`))
      ) {
        res.writeHead(401).end('{}');
        return;
      }
      requests.push({ method: 'POST', path, tsg });
      res.end(
        JSON.stringify({
          access_token: `mock-token-${tsg}`,
          expires_in: 3600,
          token_type: 'Bearer',
        }),
      );
      return;
    }
    const tsg = (req.headers.authorization ?? '').replace('Bearer mock-token-', '');
    if (!['100', '200'].includes(tsg)) {
      res.writeHead(401).end('{}');
      return;
    }
    requests.push({ method: req.method ?? '', path, tsg });
    const collection =
      /^\/dlp\/v2\/api\/(data-patterns|data-profiles|dictionaries|data-filtering-profiles)$/.test(
        path,
      );
    if (collection && req.method === 'GET') {
      res.end(
        JSON.stringify({
          content: [],
          totalElements: 0,
          totalPages: 0,
          number: 0,
          size: 20,
          last: true,
        }),
      );
    } else if (collection && req.method === 'POST') {
      const resource = { ...JSON.parse(body), id: `resource-${tsg}`, version: 1 };
      resources.set(`${tsg}:${path}/${resource.id}`, resource);
      res.end(JSON.stringify(resource));
    } else if (req.method === 'GET' && resources.has(`${tsg}:${path}`)) {
      res.end(JSON.stringify(resources.get(`${tsg}:${path}`)));
    } else {
      res.writeHead(404).end('{}');
    }
  });
  await new Promise<void>((done) => server.listen(0, '127.0.0.1', done));
  const base = `http://127.0.0.1:${(server.address() as AddressInfo).port}`;
  env = Object.fromEntries(
    Object.entries(process.env).filter(
      ([key]) => !key.startsWith('PANW_') && !key.startsWith('PRISMA_AIRS_'),
    ),
  );
  env.PRISMA_AIRS_TENANTS_PATH = join(directory, 'tenants.json');
  env.DOTENV_CONFIG_PATH = '/dev/null';
  env.NO_COLOR = '1';
  configs = [];
  for (const [name, tsg] of [
    ['prod', '100'],
    ['dev', '200'],
  ]) {
    const config = JSON.stringify({
      mgmtClientId: `client-${tsg}`,
      mgmtClientSecret: `FAKE-SECRET-${tsg}`,
      mgmtTsgId: tsg,
      mgmtEndpoint: `${base}/not-the-dlp-host`,
      mgmtTokenEndpoint: `${base}/oauth/token`,
      dlpEndpoint: `${base}/dlp`,
    });
    configs.push(config);
    await writeFile(join(directory, `${name}.json`), config, { mode: 0o400 });
    await cli(['tenant', 'create', name, '--config', join(directory, `${name}.json`)]);
  }
  await cli(['tenant', 'switch', 'prod']);
});

afterEach(async () => {
  await new Promise<void>((done) => server.close(() => done()));
  await rm(directory, { recursive: true, force: true });
});

describe('DLP commands use selected tenant config through real CLI and SDK OAuth', () => {
  it.each([
    'patterns',
    'profiles',
    'dictionaries',
    'filtering-profiles',
  ])('loads JSON credentials and the DLP endpoint for %s', async (group) => {
    await cli(['runtime', 'dlp', group, 'list', '--output', 'json']);
    expect(requests).toHaveLength(2);
    expect(requests[0]).toEqual({ method: 'POST', path: '/oauth/token', tsg: '100' });
    expect(requests[1]).toMatchObject({ method: 'GET', tsg: '100' });
    expect(requests[1].path).toMatch(/^\/dlp\/v2\/api\//);
  });

  it('creates and reads a pattern and advanced profile in each tenant without changing credentials', async () => {
    for (const [name, tsg] of [
      ['prod', '100'],
      ['dev', '200'],
    ]) {
      await cli(['tenant', 'switch', name]);
      const pattern = JSON.parse(
        (
          await cli([
            'runtime',
            'dlp',
            'patterns',
            'create',
            '--name',
            'dlp-test-pattern',
            '--type',
            'custom',
            '--technique',
            'regex',
            '--confidence-levels',
            'high',
            '--regex',
            'AIRS-E2E-[0-9]{6}',
            '--output',
            'json',
          ])
        ).stdout,
      );
      expect(pattern.id).toBe(`resource-${tsg}`);
      const stored = JSON.parse(
        (await cli(['runtime', 'dlp', 'patterns', 'get', pattern.id, '--output', 'json'])).stdout,
      );
      expect(stored.matchingRules.regexes[0].regex).toBe('AIRS-E2E-[0-9]{6}');
      const body = {
        name: 'dlp-test',
        profile_type: 'advanced',
        detection_rules: [
          {
            rule_type: 'expression_tree',
            expression_tree: {
              operator_type: 'or',
              sub_expressions: [
                {
                  rule_item: {
                    detection_technique: 'regex',
                    id: pattern.id,
                    name: pattern.name,
                    match_type: 'include',
                    confidence_level: 'high',
                    occurrence_operator_type: 'any',
                    occurrence_count: 1,
                  },
                },
              ],
            },
          },
        ],
      };
      const file = join(directory, `${name}-request.json`);
      await writeFile(file, JSON.stringify(body), { flag: 'wx', mode: 0o600 });
      const profile = JSON.parse(
        (
          await cli([
            'runtime',
            'dlp',
            'profiles',
            'create',
            '--body-file',
            file,
            '--output',
            'json',
          ])
        ).stdout,
      );
      const fetched = JSON.parse(
        (await cli(['runtime', 'dlp', 'profiles', 'get', profile.id, '--output', 'json'])).stdout,
      );
      expect(fetched.detectionRules[0].expressionTree.subExpressions[0].ruleItem.id).toBe(
        pattern.id,
      );
      expect(requests.slice(-8).every((r) => r.tsg === tsg)).toBe(true);
    }
    expect(await readFile(join(directory, 'prod.json'), 'utf8')).toBe(configs[0]);
    expect(await readFile(join(directory, 'dev.json'), 'utf8')).toBe(configs[1]);
  }, 60000);

  it('rejects mixed environment credentials before any OAuth or resource write', async () => {
    const result = await cli(
      ['runtime', 'dlp', 'patterns', 'create', '--name', 'do-not-create', '--regex', 'example'],
      { PANW_MGMT_CLIENT_ID: 'wrong-client' },
      2,
    );
    expect(result.stderr).toContain('Named tenant selection conflicts');
    expect(requests).toEqual([]);
  });

  it('still supports an explicitly selected config file', async () => {
    await cli(['runtime', 'dlp', 'profiles', 'list', '--output', 'json'], {
      PRISMA_AIRS_CONFIG_PATH: join(directory, 'dev.json'),
    });
    expect(requests.map((r) => r.tsg)).toEqual(['200', '200']);
  });
});
