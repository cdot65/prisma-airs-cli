import { execFile } from 'node:child_process';
import { mkdtemp, readdir, readFile, rm, writeFile } from 'node:fs/promises';
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
let response: Record<string, unknown>;
let requests: Array<{ method: string; body: string }>;

async function cli(args: string[], code: number, debugPosition?: 'before' | 'after') {
  const entry = process.env.AIRS_CLI_ENTRY
    ? [resolve(process.env.AIRS_CLI_ENTRY)]
    : ['--import', resolve('node_modules/tsx/dist/loader.mjs'), resolve('src/cli/index.ts')];
  let result: { stdout: string; stderr: string; code?: number };
  try {
    result = await exec(
      process.execPath,
      [
        ...entry,
        ...(debugPosition === 'before' ? ['--debug'] : []),
        'runtime',
        'dlp',
        'dictionaries',
        ...args,
        ...(debugPosition === 'after' ? ['--debug'] : []),
      ],
      {
        cwd: directory,
        env,
        timeout: 20000,
      },
    );
  } catch (error) {
    result = error as typeof result;
  }
  expect(result.code ?? 0, result.stderr).toBe(code);
  expect(result.stdout + result.stderr).not.toMatch(/PRIVATE-KEYWORD|FAKE-SECRET|mock-token/);
  return result;
}

function uploadArgs(command = 'create') {
  return [
    command,
    ...(command === 'replace' ? ['dictionary-id'] : []),
    '--name',
    'test-dictionary',
    '--category',
    'Confidential',
    '--region',
    'United States',
    '--file',
    join(directory, 'keywords.txt'),
    '--output',
    'json',
  ];
}

beforeEach(async () => {
  directory = await mkdtemp(join(tmpdir(), 'airs-dictionary-errors-'));
  requests = [];
  response = {
    title: 'Bad Request',
    status: 400,
    errors: [{ property: 'originalFileName', reason: 'must not be blank' }],
  };
  server = createServer(async (req, res) => {
    const chunks: Buffer[] = [];
    for await (const chunk of req) chunks.push(Buffer.from(chunk));
    const body = Buffer.concat(chunks).toString();
    if (req.url === '/oauth/token') {
      res.setHeader('Content-Type', 'application/json');
      res.end(JSON.stringify({ access_token: 'mock-token', expires_in: 3600 }));
      return;
    }
    requests.push({ method: req.method ?? '', body });
    res.setHeader('Content-Type', 'application/problem+json');
    res.writeHead(400).end(JSON.stringify(response));
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
  await writeFile(join(directory, 'keywords.txt'), 'PRIVATE-KEYWORD\n', { mode: 0o600 });
});

afterEach(async () => {
  await new Promise<void>((done) => server.close(() => done()));
  await rm(directory, { recursive: true, force: true });
});

describe('dictionary CLI errors through the real SDK and HTTP transport', () => {
  it.each([
    'create',
    'replace',
  ])('renders safe field-level %s errors with operational exit 1', async (command) => {
    response.detail = 'Rejected PRIVATE-KEYWORD with FAKE-SECRET';
    response.request = { keywords: ['PRIVATE-KEYWORD'], token: 'mock-token' };
    response.errors = [
      {
        property: 'originalFileName',
        reason: 'must not be blank',
        rejectedValue: 'PRIVATE-KEYWORD',
      },
      { property: 'keywords', reason: 'contains PRIVATE-KEYWORD' },
    ];
    const result = await cli(uploadArgs(command), 1);
    expect(result.stderr).toContain('originalFileName');
    expect(result.stderr).toContain('must not be blank');
    expect(result.stderr).not.toContain('entitlement');
    expect(result.stdout).toBe('');
    expect(requests).toHaveLength(1);
    expect(requests[0].method).toBe(command === 'create' ? 'POST' : 'PUT');
    expect(requests[0].body).toContain('"region_name":"United States"');
    expect(requests[0].body).toContain('name="json"');
    expect(requests[0].body).toContain('Content-Type: application/json');
    expect(requests[0].body).toContain('name="file"');
  });

  it('offers a neutral region hint for detail-free 400 without changing the supplied region', async () => {
    response = { title: 'Bad Request', status: 400 };
    const args = uploadArgs();
    args[args.indexOf('United States')] = 'GLOBAL';
    const result = await cli(args, 1);
    expect(result.stderr).toContain('SCM region display name');
    expect(result.stderr).toContain('United States');
    expect(result.stderr).not.toMatch(/license|entitlement/i);
    expect(requests[0].body).toContain('"region_name":"GLOBAL"');
  });

  it('classifies PATCH API rejection as operational and preserves safe validation detail', async () => {
    const result = await cli(
      [
        'patch',
        'dictionary-id',
        '--set',
        'name=test',
        '--set',
        'category=Confidential',
        '--set',
        'original_file_name=keywords.txt',
      ],
      1,
    );
    expect(result.stderr).toContain('originalFileName');
    expect(result.stderr).toContain('must not be blank');
    expect(requests.map((request) => request.method)).toEqual(['PATCH']);
  });

  it.each([
    'create',
    'replace',
  ])('keeps missing %s flags as usage exit 2 before a request', async (command) => {
    const result = await cli([command, ...(command === 'replace' ? ['dictionary-id'] : [])], 2);
    expect(result.stderr).toContain('--name, --category, --region, and --file are required');
    expect(requests).toEqual([]);
  });

  it.each([
    'create',
    'replace',
    'patch',
  ])('does not echo malformed %s metadata in usage errors', async (command) => {
    const path = join(directory, 'metadata.json');
    await writeFile(path, '{"keywords": PRIVATE-KEYWORD}', { mode: 0o600 });
    const args =
      command === 'patch'
        ? ['patch', 'dictionary-id', '--body-file', path]
        : [...uploadArgs(command), '--metadata-file', path];
    const result = await cli(args, 2);
    expect(result.stderr).toContain('Invalid JSON in dictionary metadata file');
    expect(requests).toEqual([]);
  });

  it('rejects nonobject metadata before upload', async () => {
    const path = join(directory, 'metadata.json');
    await writeFile(path, '["PRIVATE-KEYWORD"]', { mode: 0o600 });
    const result = await cli([...uploadArgs(), '--metadata-file', path], 2);
    expect(result.stderr).toContain('Dictionary metadata must be a JSON object');
    expect(requests).toEqual([]);
  });

  it('keeps SDK request-schema validation as usage exit 2 without echoing metadata values', async () => {
    const result = await cli(
      [
        'patch',
        'dictionary-id',
        '--set',
        'name=test',
        '--set',
        'category=PRIVATE-KEYWORD',
        '--set',
        'original_file_name=keywords.txt',
      ],
      2,
    );
    expect(result.stderr).toContain('Invalid dictionary metadata');
    expect(requests).toEqual([]);
  });

  it('keeps missing PATCH metadata fields as usage exit 2', async () => {
    const result = await cli(['patch', 'dictionary-id', '--set', 'name=test'], 2);
    expect(result.stderr).toContain('Invalid dictionary metadata');
    expect(requests).toEqual([]);
  });

  it.each([
    'before',
    'after',
  ] as const)('omits all DLP bodies with global --debug %s the subcommand', async (position) => {
    response.detail = 'Rejected PRIVATE-KEYWORD with FAKE-SECRET';
    response.keywords = ['PRIVATE-KEYWORD'];
    env.PANW_AI_SEC_DEBUG = '1';
    env.PANW_AI_SEC_DEBUG_BODY = '1';
    const result = await cli(uploadArgs(), 1, position);
    expect(result.stderr).toContain('[BODY OMITTED]');
    expect(result.stderr).toContain('← 400');
    const logs = (await readdir(directory)).filter((file) => file.startsWith('debug-api-'));
    expect(logs).toHaveLength(1);
    const log = await readFile(join(directory, logs[0]), 'utf8');
    expect(log).not.toMatch(/PRIVATE-KEYWORD|FAKE-SECRET|mock-token|Rejected/);
    // The CLI logger filters non-PANW hosts; SDK diagnostics above exercise real HTTP.
    // dlp-debug.spec.ts checks the same program hook against a PANW URL and JSONL bodies.
    expect(log).toBe('');
  });

  it('does not echo malformed patch flags', async () => {
    const result = await cli(['patch', 'dictionary-id', '--set', 'PRIVATE-KEYWORD'], 2);
    expect(result.stderr).toContain('Invalid dictionary patch flags');
    expect(requests).toEqual([]);
  });

  it('rejects conflicting patch flags before a request', async () => {
    const result = await cli(
      ['patch', 'dictionary-id', '--body-file', 'unused.json', '--set', 'name=test'],
      2,
    );
    expect(result.stderr).toContain('mutually exclusive');
    expect(requests).toEqual([]);
  });
});
