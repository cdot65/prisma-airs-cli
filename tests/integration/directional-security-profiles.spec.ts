import { execFile } from 'node:child_process';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { createServer, type Server } from 'node:http';
import type { AddressInfo } from 'node:net';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { promisify } from 'node:util';
import type { SecurityProfile } from '@cdot65/prisma-airs-sdk';
import { load as yaml } from 'js-yaml';
import fixture from '../fixtures/directional-security-profile.json';
import { required } from '../helpers/required.js';

const exec = promisify(execFile);
let directory: string;
let server: Server;
let env: NodeJS.ProcessEnv;
let profile: SecurityProfile;
let requests: Array<{
  method: string;
  path: string;
  body?: SecurityProfile;
}>;
let topics: Array<{
  topic_id: string;
  topic_name: string;
  revision: number;
  description: string;
  examples: string[];
}>;
async function cli(args: string[], expected = 0) {
  const executable = process.env.AIRS_CLI_ENTRY
    ? [resolve(process.env.AIRS_CLI_ENTRY)]
    : ['--import', resolve('node_modules/tsx/dist/loader.mjs'), resolve('src/cli/index.ts')];
  let result: {
    stdout: string;
    stderr: string;
    code?: number;
  };
  try {
    result = await exec(process.execPath, [...executable, ...args], {
      cwd: directory,
      env,
      timeout: 20000,
    });
  } catch (err) {
    result = err as typeof result;
  }
  expect(result.code ?? 0, result.stderr).toBe(expected);
  return result;
}
beforeEach(async () => {
  directory = await mkdtemp(join(tmpdir(), 'airs-directional-'));
  profile = structuredClone(fixture);
  topics = [
    {
      topic_id: '00000000-0000-4000-8000-000000000001',
      topic_name: 'Restricted',
      revision: 3,
      description: 'Fixture',
      examples: ['Example'],
    },
  ];
  requests = [];
  server = createServer(async (req, res) => {
    const chunks: Buffer[] = [];
    for await (const chunk of req) chunks.push(Buffer.from(chunk));
    const raw = Buffer.concat(chunks).toString();
    const path = new URL(required(req.url), 'http://localhost').pathname;
    const body = raw && !path.includes('/oauth/') ? JSON.parse(raw) : undefined;
    requests.push({ method: required(req.method), path, ...(body ? { body } : {}) });
    res.setHeader('Content-Type', 'application/json');
    if (path === '/oauth/token') {
      res.end(JSON.stringify({ access_token: 'fixture-token', expires_in: 3600 }));
      return;
    }
    if (req.headers.authorization !== 'Bearer fixture-token') {
      res.statusCode = 401;
      res.end('{}');
      return;
    }
    if (req.method === 'GET' && path.includes('/profiles/')) {
      const retrieved = structuredClone(profile);
      delete retrieved.dlp_tenant_id;
      res.end(JSON.stringify({ ai_profiles: [retrieved], next_offset: 0 }));
      return;
    }
    if (req.method === 'GET' && path.includes('/topics/')) {
      res.end(JSON.stringify({ custom_topics: topics, next_offset: 0 }));
      return;
    }
    if (['POST', 'PUT'].includes(required(req.method)) && path.includes('/profile')) {
      profile = {
        ...body,
        profile_id: fixture.profile_id,
        tsg_id: '100',
        revision: (profile.revision ?? 0) + 1,
      };
      res.end(JSON.stringify(profile));
      return;
    }
    if (req.method === 'DELETE' && path.includes('/topic/')) {
      topics = [];
      res.end(JSON.stringify('deleted'));
      return;
    }
    res.statusCode = 404;
    res.end('{}');
  });
  await new Promise<void>((done) => server.listen(0, '127.0.0.1', done));
  const endpoint = `http://127.0.0.1:${(server.address() as AddressInfo).port}`;
  env = {
    ...process.env,
    PRISMA_AIRS_TENANTS_PATH: join(directory, 'tenants.json'),
    NO_COLOR: '1',
  };
  delete env.PRISMA_AIRS_CONFIG_PATH;
  await writeFile(
    join(directory, 'tenant.json'),
    JSON.stringify({
      mgmtTsgId: '100',
      mgmtClientId: 'fixture-client',
      mgmtClientSecret: 'fixture-secret',
      mgmtEndpoint: endpoint,
      mgmtTokenEndpoint: `${endpoint}/oauth/token`,
    }),
    { mode: 0o400 },
  );
  await cli(['tenant', 'create', 'fixture', '--config', './tenant.json']);
  await cli(['tenant', 'switch', 'fixture']);
  await writeFile(join(directory, 'profile.json'), JSON.stringify(fixture));
  requests = [];
});
afterEach(async () => {
  await new Promise<void>((done) => server.close(() => done()));
  await rm(directory, { recursive: true, force: true });
});
it('preserves the complete JSON create fixture and get/list policy in JSON and YAML', async () => {
  const result = await cli([
    'runtime',
    'profiles',
    'create',
    '--config',
    './profile.json',
    '--output',
    'json',
  ]);
  expect(
    required(
      requests.find((request) => request.method === 'POST' && !request.path.includes('/oauth/')),
    ).body,
  ).toEqual(fixture);
  expect(JSON.parse(result.stdout).policy).toEqual(fixture.policy);
  expect(JSON.parse(result.stdout).dlpTenantId).toBe(fixture.dlp_tenant_id);
  for (const output of ['json', 'yaml']) {
    const get = await cli(['runtime', 'profiles', 'get', fixture.profile_id, '--output', output]);
    const parsed =
      output === 'json' ? JSON.parse(get.stdout) : (yaml(get.stdout) as SecurityProfile);
    expect(parsed.policy).toEqual(fixture.policy);
    expect(parsed).not.toHaveProperty('dlpTenantId');
  }
  const list = await cli(['runtime', 'profiles', 'list', '--output', 'json']);
  expect(JSON.parse(list.stdout)[0].policy).toEqual(fixture.policy);
});
it('updates response detectors and masking while preserving prompt/tool settings, metadata and UUID', async () => {
  profile.active = false;
  const prior = structuredClone(profile);
  await cli([
    'runtime',
    'profiles',
    'update',
    fixture.profile_id,
    '--direction',
    'response',
    '--toxic-content',
    'alert',
    '--malicious-code',
    'allow',
    '--no-mask-data-inline',
    '--no-enable-full-conversation-inspection',
    '--output',
    'json',
  ]);
  const write = required(requests.find((request) => request.method === 'PUT'));
  expect(write.path).toContain(fixture.profile_id);
  expect(required(write.body).active).toBe(false);
  const ai = required(required(profile.policy)['ai-security-profiles'])[0];
  for (const direction of ['prompt', 'tool-call', 'tool-response'] as const)
    expect(required(ai['content-type-configurations'])[direction]).toEqual(
      required(
        required(required(prior.policy)['ai-security-profiles'])[0]['content-type-configurations'],
      )[direction],
    );
  expect(
    required(required(required(ai['content-type-configurations']).response)['app-protection'])[
      'malicious-code-protection'
    ],
  ).toEqual({ name: 'malicious-code', action: 'allow', severity: 'high' });
  expect(
    required(required(required(ai['content-type-configurations']).response)['data-protection'])[
      'data-leak-detection'
    ],
  ).toMatchObject({ action: 'block', 'mask-data-inline': false });
  expect(required(ai['model-configuration'])['enable-full-conversation-inspection']).toBe(false);
  expect(await readFile(join(directory, 'tenant.json'), 'utf-8')).toContain('fixture-secret');
});
it.each([
  'directional',
  'legacy',
])('global-only %s updates preserve all protections', async (layout) => {
  if (layout === 'legacy')
    profile.policy = {
      'ai-security-profiles': [
        {
          'model-type': 'default',
          'model-configuration': {
            ...fixture.policy['ai-security-profiles'][0]['content-type-configurations'].prompt,
            latency: { 'inline-timeout-action': 'block', 'max-inline-latency': 5 },
          },
        },
      ],
    };
  const before = structuredClone(profile.policy);
  await cli([
    'runtime',
    'profiles',
    'update',
    fixture.profile_id,
    '--max-inline-latency',
    '9',
    '--no-mask-data-in-storage',
    '--output',
    'json',
  ]);
  const expected = structuredClone(before);
  required(
    required(required(required(expected)['ai-security-profiles'])[0]['model-configuration'])
      .latency,
  )['max-inline-latency'] = 9;
  required(required(required(expected)['ai-security-profiles'])[0]['model-configuration'])[
    'mask-data-in-storage'
  ] = false;
  expect(profile.policy).toEqual(expected);
});
it.each([
  ['create', '--config', './profile.json', '--direction', 'prompt'],
  ['create', '--name', 'Bad', '--direction', 'invalid'],
  ['create', '--name', 'Bad', '--max-inline-latency', '1.5'],
  ['create', '--name', 'Bad', '--max-inline-latency', '5garbage'],
  ['update', fixture.profile_id, '--direction', 'invalid'],
  ['update', fixture.profile_id, '--ai-profile-index', '-1'],
  ['update', fixture.profile_id, '--max-inline-latency', 'NaN'],
])('refuses locally invalid flags before OAuth: %j', async (...args) => {
  await cli(['runtime', 'profiles', ...args], 2);
  expect(requests).toEqual([]);
});
it.each([
  'create',
  'update',
])('rejects malformed directional JSON on %s before OAuth', async (command) => {
  const bad = structuredClone(fixture);
  Reflect.set(
    bad.policy['ai-security-profiles'][0]['content-type-configurations'].response[
      'model-protection'
    ][0],
    'severity',
    123,
  );
  await writeFile(join(directory, 'bad.json'), JSON.stringify(bad));
  await cli(
    [
      'runtime',
      'profiles',
      command,
      ...(command === 'update' ? [fixture.profile_id] : []),
      '--config',
      './bad.json',
    ],
    2,
  );
  expect(requests).toEqual([]);
});
it('refuses ambiguous directional edits after reading without submitting a write', async () => {
  await cli(['runtime', 'profiles', 'update', fixture.profile_id, '--toxic-content', 'alert'], 2);
  expect(requests.some((request) => request.method === 'PUT')).toBe(false);
  expect(requests.some((request) => request.method === 'GET')).toBe(true);
});
it('rejects legacy direction updates and accepts a deliberate complete JSON replacement', async () => {
  profile.policy = {
    'ai-security-profiles': [
      {
        'model-configuration': {
          'model-protection': [{ name: 'prompt-injection', action: 'block' }],
        },
      },
    ],
  };
  await cli(
    [
      'runtime',
      'profiles',
      'update',
      fixture.profile_id,
      '--direction',
      'response',
      '--toxic-content',
      'alert',
    ],
    2,
  );
  expect(requests.some((request) => request.method === 'PUT')).toBe(false);
  await cli([
    'runtime',
    'profiles',
    'update',
    fixture.profile_id,
    '--config',
    './profile.json',
    '--output',
    'json',
  ]);
  expect(profile.policy).toEqual(fixture.policy);
});
it.each([
  'directional',
  'legacy',
])('applies and reverts %s topics without touching other protections', async (layout) => {
  if (layout === 'legacy')
    profile.policy = {
      'ai-security-profiles': [{ 'model-configuration': { 'model-protection': [] } }],
    };
  const selector = layout === 'directional' ? ['--direction', 'response'] : [];
  const before = structuredClone(profile.policy);
  await cli([
    'runtime',
    'topics',
    'apply',
    '--profile',
    fixture.profile_name,
    '--name',
    'Restricted',
    ...selector,
    '--output',
    'json',
  ]);
  const ai = required(profile.policy?.['ai-security-profiles'])[0];
  const protection =
    layout === 'directional'
      ? required(ai['content-type-configurations']?.response)
      : required(ai['model-configuration']);
  expect(required(protection['model-protection']).at(-1)).toMatchObject({
    name: 'topic-guardrails',
    'topic-list': [
      {
        action: 'block',
        topic: [{ topic_id: '00000000-0000-4000-8000-000000000001', revision: 3 }],
      },
    ],
  });
  if (layout === 'directional')
    for (const direction of ['prompt', 'tool-call', 'tool-response'] as const)
      expect(required(ai['content-type-configurations'])[direction]).toEqual(
        required(required(before?.['ai-security-profiles'])[0]['content-type-configurations'])[
          direction
        ],
      );
  await cli([
    'runtime',
    'topics',
    'revert',
    '--profile',
    fixture.profile_name,
    '--name',
    'Restricted',
    ...selector,
    '--force',
    '--output',
    'json',
  ]);
  expect(
    requests.some((request) => request.method === 'DELETE' && request.path.includes('/force')),
  ).toBe(false);
  expect(topics).toEqual([]);
});

it('refuses ambiguous topic apply and refuses revert when another direction still references the topic', async () => {
  await cli(
    [
      'runtime',
      'topics',
      'apply',
      '--profile',
      fixture.profile_name,
      '--name',
      'Restricted',
      '--output',
      'json',
    ],
    2,
  );
  expect(requests.filter((request) => request.method === 'PUT')).toEqual([]);
  for (const direction of ['prompt', 'response'] as const)
    required(
      required(profile.policy?.['ai-security-profiles'])[0]['content-type-configurations']?.[
        direction
      ],
    )['model-protection']?.push({
      name: 'topic-guardrails',
      action: 'allow',
      'topic-list': [
        {
          action: 'block',
          topic: [{ topic_id: topics[0].topic_id, topic_name: topics[0].topic_name, revision: 3 }],
        },
      ],
    });
  requests = [];
  await cli(
    [
      'runtime',
      'topics',
      'revert',
      '--profile',
      fixture.profile_name,
      '--name',
      'Restricted',
      '--direction',
      'response',
      '--force',
    ],
    2,
  );
  expect(
    requests.filter((request) => request.method === 'PUT' || request.method === 'DELETE'),
  ).toEqual([]);
});

it.each([
  'apply',
  'revert',
])('refuses %s against an absent direction or legacy layout before any write', async (command) => {
  const args = [
    'runtime',
    'topics',
    command,
    '--profile',
    fixture.profile_name,
    '--name',
    'Restricted',
    '--direction',
    'tool-response',
    ...(command === 'revert' ? ['--force'] : []),
  ];
  delete required(
    required(profile.policy?.['ai-security-profiles'])[0]['content-type-configurations'],
  )['tool-response'];
  await cli(args, 2);
  profile.policy = { 'ai-security-profiles': [{ 'model-configuration': {} }] };
  await cli(args, 2);
  expect(
    requests.filter((request) => request.method === 'PUT' || request.method === 'DELETE'),
  ).toEqual([]);
});

it('refuses revert while a retained inactive legacy block references the topic', async () => {
  const guardrail = {
    name: 'topic-guardrails',
    action: 'allow',
    'topic-list': [
      {
        action: 'block',
        topic: [{ topic_id: topics[0].topic_id, topic_name: topics[0].topic_name, revision: 3 }],
      },
    ],
  };
  const ai = required(profile.policy?.['ai-security-profiles'])[0];
  ai['model-configuration'] = {
    ...ai['model-configuration'],
    'model-protection': [structuredClone(guardrail)],
  };
  required(ai['content-type-configurations']?.response)['model-protection']?.push(
    structuredClone(guardrail),
  );
  await cli(
    [
      'runtime',
      'topics',
      'revert',
      '--profile',
      fixture.profile_name,
      '--name',
      'Restricted',
      '--direction',
      'response',
      '--force',
    ],
    2,
  );
  expect(
    requests.filter((request) => request.method === 'PUT' || request.method === 'DELETE'),
  ).toEqual([]);
});
