import { execFile } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { chmod, mkdtemp, readdir, readFile, rm, stat, writeFile } from 'node:fs/promises';
import { createServer, type Server } from 'node:http';
import type { AddressInfo } from 'node:net';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { promisify } from 'node:util';
import { load as yaml } from 'js-yaml';

const exec = promisify(execFile);
type Resource = Record<string, unknown>;
let directory: string;
let server: Server;
let env: NodeJS.ProcessEnv;
let initialConfigs: Buffer[];
let inventories: Record<string, { profiles: Resource[]; topics: Resource[] }>;
let requests: Array<{ method: string; path: string; tsg: string }>;
const digest = (v: Buffer) => createHash('sha256').update(v).digest('hex');
const sourceTopicId = '00000000-0000-4000-8000-000000000001';
const sourceProfileId = '00000000-0000-4000-8000-000000000002';

async function cli(args: string[], extraEnv: NodeJS.ProcessEnv = {}, expected = 0) {
  const executable = process.env.AIRS_CLI_ENTRY
    ? [resolve(process.env.AIRS_CLI_ENTRY)]
    : ['--import', resolve('node_modules/tsx/dist/loader.mjs'), resolve('src/cli/index.ts')];
  let result: { stdout: string; stderr: string; code?: number };
  try {
    result = await exec(process.execPath, [...executable, ...args], {
      cwd: directory,
      env: { ...env, ...extraEnv },
      timeout: 20000,
      maxBuffer: 2 * 1024 * 1024,
    });
  } catch (error) {
    result = error as typeof result;
  }
  expect(result.code ?? 0, result.stderr).toBe(expected);
  expect(`${result.stdout}${result.stderr}`).not.toContain('FAKE-SECRET');
  return result;
}

beforeEach(async () => {
  directory = await mkdtemp(join(tmpdir(), 'airs-tenant-cli-'));
  requests = [];
  inventories = {
    '100': {
      profiles: [
        {
          profile_id: sourceProfileId,
          profile_name: 'Production',
          revision: 2,
          tsg_id: '100',
          active: true,
          policy: {
            'ai-security-profiles': [
              {
                'model-configuration': {
                  'model-protection': [
                    {
                      name: 'topic-guardrail',
                      'topic-list': [
                        {
                          action: 'block',
                          topic: [
                            { topic_id: sourceTopicId, topic_name: 'Restricted', revision: 3 },
                          ],
                        },
                      ],
                    },
                  ],
                },
              },
            ],
            'dlp-data-profiles': [],
          },
        },
      ],
      topics: [
        {
          topic_id: sourceTopicId,
          topic_name: 'Restricted',
          revision: 3,
          description: 'Restricted business topics',
          examples: ['Synthetic example'],
          active: true,
        },
      ],
    },
    '200': { profiles: [], topics: [] },
  };
  server = createServer(async (req, res) => {
    try {
      const chunks: Buffer[] = [];
      for await (const chunk of req) chunks.push(Buffer.from(chunk));
      const body = Buffer.concat(chunks).toString();
      const url = new URL(req.url ?? '/', 'http://localhost');
      res.setHeader('Content-Type', 'application/json');
      if (url.pathname === '/oauth/token') {
        const form = new URLSearchParams(body);
        const tsg = form.get('scope')?.replace('tsg_id:', '') ?? '';
        const basic = Buffer.from(
          (req.headers.authorization ?? '').replace('Basic ', ''),
          'base64',
        ).toString();
        if (
          !inventories[tsg] ||
          (basic !== `client-${tsg}:FAKE-SECRET-${tsg}` &&
            !(
              form.get('client_id') === `client-${tsg}` &&
              form.get('client_secret') === `FAKE-SECRET-${tsg}`
            ))
        ) {
          res.statusCode = 401;
          res.end('{}');
          return;
        }
        requests.push({ method: 'POST', path: url.pathname, tsg });
        res.end(
          JSON.stringify({
            access_token: `mock-token-${tsg}`,
            token_type: 'Bearer',
            expires_in: 3600,
          }),
        );
        return;
      }
      const tsg = (req.headers.authorization ?? '').replace('Bearer mock-token-', '');
      const inventory = inventories[tsg];
      if (!inventory) {
        res.statusCode = 401;
        res.end('{}');
        return;
      }
      requests.push({ method: req.method ?? '', path: url.pathname, tsg });
      if (req.method === 'GET' && /\/(profiles|topics)\/tsg\//.test(url.pathname)) {
        if (!url.pathname.endsWith(`/${tsg}`)) {
          res.statusCode = 403;
          res.end('{}');
          return;
        }
        res.end(
          JSON.stringify(
            url.pathname.includes('/profiles/')
              ? { ai_profiles: inventory.profiles, next_offset: 0 }
              : { custom_topics: inventory.topics, next_offset: 0 },
          ),
        );
        return;
      }
      if (req.method === 'POST' && /\/v1\/mgmt\/(profile|topic)$/.test(url.pathname)) {
        const data = JSON.parse(body);
        const topic = url.pathname.endsWith('/topic');
        if ('tsg_id' in data || 'profile_id' in data || 'topic_id' in data || 'revision' in data) {
          res.statusCode = 400;
          res.end('{}');
          return;
        }
        const created = {
          ...data,
          [topic ? 'topic_id' : 'profile_id']: randomUUID(),
          revision: 1,
          ...(!topic ? { tsg_id: tsg } : {}),
        };
        inventory[topic ? 'topics' : 'profiles'].push(created);
        res.end(JSON.stringify(created));
        return;
      }
      res.statusCode = 404;
      res.end('{}');
    } catch {
      res.statusCode = 500;
      res.end('{}');
    }
  });
  await new Promise<void>((done) => server.listen(0, '127.0.0.1', done));
  const base = `http://127.0.0.1:${(server.address() as AddressInfo).port}`;
  env = Object.fromEntries(
    Object.entries(process.env).filter(
      ([key]) =>
        !key.startsWith('PANW_') && !['PRISMA_AIRS_CONFIG_PATH', 'FORCE_COLOR'].includes(key),
    ),
  );
  env.PRISMA_AIRS_TENANTS_PATH = join(directory, 'state', 'tenants.json');
  env.NO_COLOR = '1';
  initialConfigs = [];
  for (const tsg of ['100', '200']) {
    const content = Buffer.from(
      JSON.stringify({
        mgmtTsgId: tsg,
        mgmtClientId: `client-${tsg}`,
        mgmtClientSecret: `FAKE-SECRET-${tsg}`,
        mgmtEndpoint: base,
        mgmtTokenEndpoint: `${base}/oauth/token`,
      }),
    );
    await writeFile(join(directory, `${tsg}.json`), content, { mode: 0o400 });
    initialConfigs.push(content);
  }
});

afterEach(async () => {
  await new Promise<void>((done) => server.close(() => done()));
  await rm(directory, { recursive: true, force: true });
});

describe('real CLI + SDK OAuth with isolated source and destination tenants', () => {
  it('preserves Basic DLP and explicitly converts custom DLP through the actual CLI', async () => {
    const policy = (custom: boolean) => ({
      'ai-security-profiles': [
        {
          'model-type': 'default',
          'model-configuration': {
            'data-protection': {
              'data-leak-detection': {
                member: [
                  {
                    text: custom ? 'Custom confidential' : 'sensitive content',
                    id: custom ? 'source-only-dlp' : '',
                    version: '2',
                  },
                ],
                action: 'block',
                'mask-data-inline': true,
              },
            },
          },
        },
      ],
      'dlp-data-profiles': [],
    });
    inventories['100'].profiles = ['Basic', 'Custom'].map((name) => ({
      profile_name: name,
      profile_id: `source-${name}`,
      tsg_id: '100',
      revision: 1,
      active: true,
      policy: policy(name === 'Custom'),
    }));
    await cli(['tenant', 'create', 'source', '--config', './100.json']);
    await cli(['tenant', 'create', 'destination', '--config', './200.json']);
    await cli(['tenant', 'switch', 'source']);
    const backup = JSON.parse(
      (await cli(['runtime', 'profiles', 'backup', '--output', 'json'])).stdout,
    )[0];
    await cli(['tenant', 'switch', 'destination']);
    const original = await readFile(backup.file);
    expect(
      (await cli(['runtime', 'profiles', 'restore', backup.file, '--dry-run'], {}, 1)).stderr,
    ).toContain('--dlp-map');
    const args = ['runtime', 'profiles', 'restore', backup.file, '--on-missing-dlp', 'basic'];
    for (const format of ['pretty', 'table', 'markdown', 'csv', 'json', 'yaml']) {
      const dry = await cli([...args, '--dry-run', '--output', format, '--quiet']);
      expect(dry.stderr).toContain('Custom rules are not preserved');
      expect(dry.stdout).toContain('Custom confidential');
      if (format === 'json') expect(JSON.parse(dry.stdout)[0].dlpFallbacks).toHaveLength(1);
    }
    expect(inventories['200'].profiles).toHaveLength(0);
    const restored = JSON.parse(
      (await cli([...args, '--force', '--expect-tsg', '200', '--output', 'json'])).stdout,
    )[0];
    expect(restored.complete).toBe(true);
    expect(restored.dlpFallbacks[0].profile).toBe('Custom');
    expect(inventories['200'].profiles).toHaveLength(2);
    for (const result of inventories['200'].profiles) expect(result.policy).toEqual(policy(false));
    expect(await readFile(backup.file)).toEqual(original);
    expect(requests.some((request) => request.path.includes('data-profiles'))).toBe(false);
    expect(await readFile(join(directory, '100.json'))).toEqual(initialConfigs[0]);
    expect(await readFile(join(directory, '200.json'))).toEqual(initialConfigs[1]);
  }, 60000);

  it('registers, backs up JSON/YAML, switches, plans, restores fresh IDs and retains source configs', async () => {
    await cli(['tenant', 'create', 'source', '--config', './100.json']);
    await cli(['tenant', 'create', 'destination', '--config', './200.json']);
    await cli(['tenant', 'switch', 'source']);
    const listed = JSON.parse((await cli(['tenant', 'list', '--output', 'json'])).stdout);
    expect(listed.find((t: Resource) => t.active)?.name).toBe('source');
    for (const format of ['pretty', 'table', 'markdown', 'csv', 'json', 'yaml']) {
      const read = await cli(['tenant', 'read', '--output', format]);
      expect(`${read.stdout}${read.stderr}`).toContain('[REDACTED]');
    }
    const exported = JSON.parse(
      (await cli(['runtime', 'profiles', 'backup', '--all', '--output', 'json'])).stdout,
    )[0];
    expect(exported.profiles).toBe(1);
    expect(exported.topics).toBe(1);
    expect(exported.file.startsWith(directory)).toBe(true);
    expect((await stat(exported.file)).mode & 0o777).toBe(0o600);
    await cli([
      'runtime',
      'profiles',
      'backup',
      'Production',
      '--file-format',
      'yaml',
      '--output-file',
      './backup.yaml',
      '--output',
      'yaml',
    ]);
    const original = await readFile(exported.file);
    await cli(['runtime', 'profiles', 'backup', '--output-file', exported.file], {}, 1);
    expect(await readFile(exported.file)).toEqual(original);
    expect(yaml(await readFile(join(directory, 'backup.yaml'), 'utf8'))).toMatchObject({
      source: { tsgId: '100' },
    });
    await cli(['tenant', 'switch', 'destination']);
    const before = requests.length;
    const dry = JSON.parse(
      (
        await cli([
          'runtime',
          'profiles',
          'restore',
          exported.file,
          '--dry-run',
          '--output',
          'json',
        ])
      ).stdout,
    )[0];
    expect(dry).toMatchObject({ sourceTsgId: '100', destinationTsgId: '200', dryRun: true });
    expect(
      requests.slice(before).filter((r) => r.method !== 'GET' && r.path !== '/oauth/token'),
    ).toEqual([]);
    const result = JSON.parse(
      (
        await cli([
          'runtime',
          'profiles',
          'restore',
          './backup.yaml',
          '--force',
          '--expect-tsg',
          '200',
          '--output',
          'json',
        ])
      ).stdout,
    )[0];
    expect(result.complete).toBe(true);
    expect(result.profiles[0].action).toBe('created');
    const restored = inventories['200'].profiles[0];
    expect(restored.profile_id).not.toBe(sourceProfileId);
    expect(restored.tsg_id).toBe('200');
    expect(JSON.stringify(restored.policy)).not.toContain(sourceTopicId);
    expect(JSON.stringify(restored.policy)).toContain(
      String(inventories['200'].topics[0].topic_id),
    );
    await cli(['runtime', 'profiles', 'restore', exported.file, '--dry-run'], {}, 1);
    await cli([
      'runtime',
      'profiles',
      'restore',
      exported.file,
      '--on-conflict',
      'skip',
      '--output',
      'json',
    ]);
    expect(inventories['200'].profiles).toHaveLength(1);
    const beforeVerify = requests.length;
    const verified = JSON.parse(
      (
        await cli([
          'runtime',
          'profiles',
          'restore',
          exported.file,
          '--on-conflict',
          'verify',
          '--output',
          'json',
        ])
      ).stdout,
    )[0];
    expect(verified.complete).toBe(true);
    expect(verified.profiles[0].action).toBe('verified');
    expect(
      requests.slice(beforeVerify).filter((r) => r.method !== 'GET' && r.path !== '/oauth/token'),
    ).toEqual([]);
    for (const format of ['pretty', 'table', 'markdown']) {
      const rendered = (
        await cli([
          'runtime',
          'profiles',
          'restore',
          exported.file,
          '--on-conflict',
          'verify',
          '--dry-run',
          '--output',
          format,
        ])
      ).stdout;
      expect(rendered).toContain('Profiles (1)');
      expect(rendered).not.toContain('[{"name":');
      expect(Math.max(...rendered.split('\n').map((line) => line.length))).toBeLessThan(160);
    }
    expect(inventories['100'].profiles[0].profile_id).toBe(sourceProfileId);
    await cli(['tenant', 'delete', 'destination', '--force'], {}, 1);
    await cli(['tenant', 'switch', 'default']);
    await cli(['tenant', 'delete', 'destination', '--force']);
    for (const [i, tsg] of ['100', '200'].entries())
      expect(digest(await readFile(join(directory, `${tsg}.json`)))).toBe(
        digest(initialConfigs[i]),
      );
    expect(await readFile(String(env.PRISMA_AIRS_TENANTS_PATH), 'utf8')).not.toContain(
      'FAKE-SECRET',
    );
  }, 60000);

  it('blocks wrong-tenant, malformed input and debug leakage before authentication', async () => {
    await cli(['tenant', 'create', 'source', '--config', './100.json']);
    await cli(['tenant', 'switch', 'source']);
    await cli(['runtime', 'profiles', 'backup', '--debug'], {}, 2);
    await cli(['runtime', 'profiles', 'backup'], { PANW_AI_SEC_DEBUG: 'true' }, 2);
    await cli(['runtime', 'profiles', 'backup', '--file-format', 'exe'], {}, 2);
    await cli(['runtime', 'profiles', 'backup', '--max-pages', '0'], {}, 2);
    await writeFile(join(directory, 'invalid.json'), '{"FAKE-SECRET":invalid');
    await cli(['runtime', 'profiles', 'restore', './invalid.json', '--dry-run'], {}, 2);
    await cli(['runtime', 'profiles', 'restore', './invalid.json', '--force'], {}, 2);
    expect(requests).toEqual([]);
    expect((await readdir(directory)).some((p) => p.endsWith('.jsonl'))).toBe(false);
    await cli(['runtime', 'profiles', 'backup', '--output-file', './backup.json']);
    const count = requests.length;
    await cli(
      ['runtime', 'profiles', 'restore', './backup.json', '--expect-tsg', '200', '--dry-run'],
      {},
      2,
    );
    expect(requests).toHaveLength(count);
    await cli(
      ['tenant', 'switch', 'source'],
      { PANW_MGMT_CLIENT_SECRET: 'FAKE-SECRET-override' },
      1,
    );
    await chmod(join(directory, '100.json'), 0o600);
    await writeFile(join(directory, '100.json'), '{}');
    await cli(['runtime', 'profiles', 'backup'], {}, 1);
    await cli(['tenant', 'list', '--output', 'json']);
    await cli(['tenant', 'switch', 'default']);
  }, 60000);
});
