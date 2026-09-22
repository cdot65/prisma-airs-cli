import { execFile } from 'node:child_process';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { createServer, type Server } from 'node:http';
import type { AddressInfo } from 'node:net';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { promisify } from 'node:util';
import { writeTestRegistry } from '../helpers/tenant.js';

const exec = promisify(execFile);
let directory: string;
let server: Server;
let env: NodeJS.ProcessEnv;
let requests: Array<{ method: string; path: string; body?: Record<string, unknown> }>;
let patterns: Record<string, Record<string, unknown>>;

async function cli(args: string[], code = 0) {
  const entry = process.env.AIRS_CLI_ENTRY
    ? [resolve(process.env.AIRS_CLI_ENTRY)]
    : ['--import', resolve('node_modules/tsx/dist/loader.mjs'), resolve('src/cli/index.ts')];
  let result: { stdout: string; stderr: string; code?: number };
  try {
    result = await exec(process.execPath, [...entry, 'runtime', 'dlp', 'profiles', ...args], {
      cwd: directory,
      env,
      timeout: 20000,
    });
  } catch (error) {
    result = error as typeof result;
  }
  expect(result.code ?? 0, result.stderr).toBe(code);
  expect(result.stdout + result.stderr).not.toMatch(/PRIVATE-PAYLOAD|FAKE-SECRET|mock-token/);
  return result;
}

beforeEach(async () => {
  directory = await mkdtemp(join(tmpdir(), 'airs-profile-pattern-flags-'));
  requests = [];
  patterns = {
    p1: {
      id: 'p1',
      name: 'Custom Pattern',
      type: 'custom',
      version: 3,
      status: 'active',
      detection_config: { technique: 'regex', supported_confidence_levels: ['high', 'low'] },
    },
    p2: {
      id: 'p2',
      name: 'Predefined Pattern',
      type: 'predefined',
      version: 7,
      status: 'active',
      detection_config: { technique: 'weighted_regex', supported_confidence_levels: ['high'] },
    },
  };
  server = createServer(async (req, res) => {
    const chunks: Buffer[] = [];
    for await (const chunk of req) chunks.push(Buffer.from(chunk));
    const raw = Buffer.concat(chunks).toString();
    const path = new URL(req.url ?? '/', 'http://localhost').pathname;
    requests.push({
      method: req.method ?? '',
      path,
      ...(raw.startsWith('{') ? { body: JSON.parse(raw) } : {}),
    });
    res.setHeader('Content-Type', 'application/json');
    if (path === '/oauth/token') {
      res.end(JSON.stringify({ access_token: 'mock-token', expires_in: 3600 }));
      return;
    }
    const id = path.match(/\/data-patterns\/(.+)$/)?.[1];
    if (id && patterns[id] && req.method === 'GET') {
      res.end(JSON.stringify(patterns[id]));
    } else if (
      path.includes('/data-profiles') &&
      ['POST', 'PUT', 'PATCH'].includes(req.method ?? '')
    ) {
      res.end(
        JSON.stringify({
          ...JSON.parse(raw),
          id: 'profile-id',
          type: 'custom',
          version: 1,
          status: 'active',
        }),
      );
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
  env.NO_COLOR = '1';
  await writeTestRegistry(
    env.PRISMA_AIRS_TENANTS_PATH,
    [
      {
        name: 'test',
        config: {
          mgmtClientId: 'test-client',
          mgmtClientSecret: 'FAKE-SECRET',
          mgmtTsgId: '100',
          mgmtTokenEndpoint: `${base}/oauth/token`,
          dlpEndpoint: `${base}/dlp`,
        },
      },
    ],
    'test',
  );
});

afterEach(async () => {
  await new Promise<void>((done) => server.close(() => done()));
  await rm(directory, { recursive: true, force: true });
});

describe('profile pattern flags resolve real SDK references before mutation', () => {
  it.each([
    'create',
    'replace',
  ])('builds a complete mixed-technique %s expression from pattern GETs', async (command) => {
    const result = await cli([
      command,
      ...(command === 'replace' ? ['profile-id'] : []),
      '--name',
      'Mixed profile',
      '--pattern-id',
      'p1',
      '--pattern-id',
      'p2',
      '--combinator',
      'and',
      '--output',
      'json',
    ]);
    expect(JSON.parse(result.stdout).id).toBe('profile-id');
    expect(requests.map(({ method, path }) => [method, path])).toEqual([
      ['POST', '/oauth/token'],
      ['GET', '/dlp/v2/api/data-patterns/p1'],
      ['GET', '/dlp/v2/api/data-patterns/p2'],
      [
        command === 'create' ? 'POST' : 'PUT',
        `/dlp/v2/api/data-profiles${command === 'replace' ? '/profile-id' : ''}`,
      ],
    ]);
    expect(requests.at(-1)?.body).toEqual({
      name: 'Mixed profile',
      profile_type: 'advanced',
      detection_rules: [
        {
          rule_type: 'expression_tree',
          expression_tree: {
            operator_type: 'and',
            sub_expressions: ['p1', 'p2'].map((id) => ({
              rule_item: {
                id,
                name: patterns[id].name,
                version: patterns[id].version,
                detection_technique: (patterns[id].detection_config as { technique: string })
                  .technique,
                confidence_level: 'high',
                supported_confidence_levels: id === 'p1' ? ['high', 'low'] : ['high'],
                match_type: 'include',
                occurrence_operator_type: 'more_than_equal_to',
                occurrence_count: 1,
              },
            })),
          },
        },
      ],
    });
  });

  it('stops without writing when one pattern is missing', async () => {
    await cli(
      ['create', '--name', 'Missing reference', '--pattern-id', 'p1', '--pattern-id', 'missing'],
      1,
    );
    expect(requests.filter((request) => request.path.includes('/data-profiles'))).toEqual([]);
  });

  it.each([
    'deleted',
    'deprecated',
    'disabled',
  ])('stops without writing for %s references', async (status) => {
    patterns.p1.status = status;
    const result = await cli(['create', '--name', 'Retired reference', '--pattern-id', 'p1'], 1);
    expect(result.stderr).toContain('not active');
    expect(requests.filter((request) => request.path.includes('/data-profiles'))).toEqual([]);
  });

  it('rejects mismatched pattern identity', async () => {
    patterns.p1.id = 'different';
    const result = await cli(['create', '--name', 'Mismatched reference', '--pattern-id', 'p1'], 1);
    expect(result.stderr).toContain('matching ID');
    expect(requests.filter((request) => request.path.includes('/data-profiles'))).toEqual([]);
  });

  it('rejects confidence unsupported by a referenced pattern', async () => {
    const result = await cli(
      ['create', '--name', 'Low confidence', '--pattern-id', 'p2', '--confidence', 'low'],
      2,
    );
    expect(result.stderr).toContain('confidence is not supported');
    expect(requests.filter((request) => request.path.includes('/data-profiles'))).toEqual([]);
  });

  it.each(['create', 'replace'])('rejects explicit basic %s before OAuth', async (command) => {
    const result = await cli(
      [
        command,
        ...(command === 'replace' ? ['profile-id'] : []),
        '--name',
        'Basic',
        '--pattern-id',
        'p1',
        '--profile-type',
        'basic',
      ],
      2,
    );
    expect(result.stderr).toContain('Basic profile writes are unsupported');
    expect(requests).toEqual([]);
  });

  it.each([
    'create',
    'replace',
    'patch',
  ])('rejects basic raw %s bodies before OAuth', async (command) => {
    const path = join(directory, 'basic.json');
    await writeFile(path, JSON.stringify({ name: 'Basic', profile_type: 'basic' }));
    const result = await cli(
      [command, ...(command === 'create' ? [] : ['profile-id']), '--body-file', path],
      2,
    );
    expect(result.stderr).toContain('Basic profile writes are unsupported');
    expect(requests).toEqual([]);
  });

  it.each([
    ['--combinator', 'xor'],
    ['--confidence', 'unknown'],
  ])('rejects invalid %s before OAuth', async (flag, value) => {
    await cli(['create', '--name', 'Invalid', '--pattern-id', 'p1', flag, value], 2);
    expect(requests).toEqual([]);
  });

  it.each(['create', 'replace'])('accepts 32-character flag names for %s', async (command) => {
    await cli([
      command,
      ...(command === 'replace' ? ['profile-id'] : []),
      '--name',
      'A'.repeat(32),
      '--pattern-id',
      'p1',
    ]);
    expect(requests.at(-1)?.body?.name).toBe('A'.repeat(32));
  });

  it.each([
    'create',
    'replace',
  ])('rejects 33-character flag names for %s before OAuth', async (command) => {
    const result = await cli(
      [
        command,
        ...(command === 'replace' ? ['profile-id'] : []),
        '--name',
        'PRIVATE-PAYLOAD'.padEnd(33, 'A'),
        '--pattern-id',
        'p1',
      ],
      2,
    );
    expect(result.stderr).toContain('at most 32 characters');
    expect(requests).toEqual([]);
  });

  it.each([
    'create',
    'replace',
    'patch',
  ])('accepts 32-character raw-body names for %s', async (command) => {
    const path = join(directory, 'valid-name.json');
    await writeFile(
      path,
      JSON.stringify({
        name: 'A'.repeat(32),
        profile_type: 'advanced',
        detection_rules: [
          {
            rule_type: 'expression_tree',
            expression_tree: {
              operator_type: 'or',
              sub_expressions: [
                {
                  rule_item: {
                    id: 'p1',
                    name: 'Custom Pattern',
                    version: 3,
                    detection_technique: 'regex',
                    confidence_level: 'high',
                    match_type: 'include',
                    occurrence_operator_type: 'more_than_equal_to',
                    occurrence_count: 1,
                  },
                },
              ],
            },
          },
        ],
      }),
    );
    await cli([command, ...(command === 'create' ? [] : ['profile-id']), '--body-file', path]);
    expect(requests.at(-1)?.body?.name).toBe('A'.repeat(32));
  });

  it.each([
    'create',
    'replace',
    'patch',
  ])('rejects 33-character raw-body names for %s before OAuth', async (command) => {
    const path = join(directory, 'invalid-name.json');
    await writeFile(
      path,
      JSON.stringify({ name: 'PRIVATE-PAYLOAD'.padEnd(33, 'A'), profile_type: 'advanced' }),
    );
    const result = await cli(
      [command, ...(command === 'create' ? [] : ['profile-id']), '--body-file', path],
      2,
    );
    expect(result.stderr).toContain('at most 32 characters');
    expect(requests).toEqual([]);
  });

  it('sanitizes malformed raw JSON before OAuth', async () => {
    const result = await cli(['create', '--body', '{"PRIVATE-PAYLOAD":invalid}'], 2);
    expect(result.stderr).toContain('verify the file and JSON syntax');
    expect(requests).toEqual([]);
  });
});
