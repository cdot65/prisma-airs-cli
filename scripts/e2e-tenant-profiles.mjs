// Live acceptance: read-only tenant config; mutations limited to uniquely named synthetic resources.
// Run: node scripts/e2e-tenant-profiles.mjs
// AIRS_CLI_ENTRY optionally points to an independently installed CLI executable.
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { mkdtemp, readFile, stat, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';
import { ManagementClient } from '@cdot65/prisma-airs-sdk';
import { selectedTenant } from './lib/live-tenant.mjs';

const exec = promisify(execFile);
const repo = fileURLToPath(new URL('..', import.meta.url));
const source = selectedTenant().configPath;
const bytes = await readFile(source);
const config = JSON.parse(bytes.toString());
// This process uses the supplied file only; never mix an inherited endpoint/key with it.
for (const key of Object.keys(process.env)) if (key.startsWith('PANW_')) delete process.env[key];
const hash = (value) => createHash('sha256').update(value).digest('hex');
const directory = await mkdtemp(join(repo, 'artifacts/tenant-profiles-e2e-'));
const entry = process.env.AIRS_CLI_ENTRY ?? join(repo, 'dist/cli/index.js');
const env = Object.fromEntries(
  Object.entries(process.env).filter(
    ([key]) =>
      !key.startsWith('PANW_') && !['PRISMA_AIRS_CONFIG_PATH', 'FORCE_COLOR'].includes(key),
  ),
);
Object.assign(env, { PRISMA_AIRS_TENANTS_PATH: join(directory, 'tenants.json'), NO_COLOR: '1' });
const client = new ManagementClient({
  clientId: config.mgmtClientId,
  clientSecret: config.mgmtClientSecret,
  tsgId: config.mgmtTsgId,
  apiEndpoint: config.mgmtEndpoint,
  tokenEndpoint: config.mgmtTokenEndpoint,
  numRetries: 0,
});
const suffix = randomUUID().slice(0, 8);
const name = `cli-transfer-e2e-${suffix}`;
const prefix = `copy-${suffix}-`;
const ownedProfiles = new Set();
const ownedTopics = new Set();
const results = [];
const transcript = [];
async function cli(args, expected = 0) {
  let result;
  try {
    result = await exec(process.execPath, [entry, ...args], {
      cwd: directory,
      env,
      timeout: 90000,
      maxBuffer: 5 * 1024 * 1024,
    });
  } catch (error) {
    result = error;
  }
  assert.equal(
    result.code ?? 0,
    expected,
    'CLI exit status mismatch (output intentionally omitted)',
  );
  for (const secret of [
    config.mgmtClientSecret,
    config.airsApiKey,
    config.aiGwInferenceApiKey,
  ].filter(Boolean))
    assert.ok(!`${result.stdout}${result.stderr}`.includes(secret), 'Credential leaked');
  transcript.push({
    args,
    exitCode: result.code ?? 0,
    stdout: result.stdout,
    stderr: result.stderr,
  });
  return result.stdout;
}
async function check(label, operation) {
  const evidence = await operation();
  const result = { check: label, passed: true, ...evidence };
  results.push(result);
  console.log(JSON.stringify(result));
}
try {
  await check('Register and select read-only live config through isolated registry', async () => {
    await cli(['tenant', 'create', 'live', '--config', source]);
    await cli(['tenant', 'switch', 'live']);
    const tenants = JSON.parse(await cli(['tenant', 'list', '--output', 'json']));
    assert.equal(tenants.find((t) => t.active).name, 'live');
    const read = JSON.parse(await cli(['tenant', 'read', '--output', 'json']));
    assert.equal(read.find((r) => r.key === 'mgmtClientSecret').value, '[REDACTED]');
  });
  await check('Live OAuth and complete profile/topic backup', async () => {
    const output = JSON.parse(
      await cli([
        'runtime',
        'profiles',
        'backup',
        '--all',
        '--output-file',
        './all-profiles.json',
        '--output',
        'json',
      ]),
    )[0];
    assert.ok(output.profiles > 0);
    assert.equal((await stat(output.file)).mode & 0o777, 0o600);
    const dry = JSON.parse(
      await cli([
        'runtime',
        'profiles',
        'restore',
        './all-profiles.json',
        '--dry-run',
        '--on-conflict',
        'skip',
        '--output',
        'json',
      ]),
    )[0];
    assert.ok(dry.profiles.every((p) => p.action === 'skip'));
    return { profiles: output.profiles, topics: output.topics, dryRunSkipped: dry.profiles.length };
  });
  await check('Create isolated synthetic profile and topic for migration acceptance', async () => {
    const topic = await client.topics.create({
      topic_name: name,
      active: true,
      description: 'Synthetic profile migration acceptance topic about space exploration.',
      examples: ['An astronaut walks on the moon.'],
    });
    assert.ok(topic.topic_id);
    ownedTopics.add(topic.topic_id);
    const profile = await client.profiles.create({
      profile_name: name,
      active: true,
      policy: {
        'ai-security-profiles': [
          {
            'model-type': 'default',
            'model-configuration': {
              'model-protection': [
                {
                  name: 'topic-guardrail',
                  'topic-list': [
                    {
                      action: 'block',
                      topic: [
                        {
                          topic_id: topic.topic_id,
                          topic_name: topic.topic_name,
                          revision: topic.revision,
                        },
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
    });
    assert.ok(profile.profile_id);
    ownedProfiles.add(profile.profile_id);
    return { syntheticProfiles: 1, syntheticTopics: 1 };
  });
  await check(
    'JSON and YAML backup; force destination assertion; live restore verification',
    async () => {
      await cli([
        'runtime',
        'profiles',
        'backup',
        name,
        '--output-file',
        './synthetic.json',
        '--output',
        'json',
      ]);
      await cli([
        'runtime',
        'profiles',
        'backup',
        name,
        '--file-format',
        'yaml',
        '--output-file',
        './synthetic.yaml',
        '--output',
        'yaml',
      ]);
      const dry = JSON.parse(
        await cli([
          'runtime',
          'profiles',
          'restore',
          './synthetic.json',
          '--dry-run',
          '--name-prefix',
          prefix,
          '--output',
          'json',
        ]),
      )[0];
      assert.equal(dry.profiles[0].action, 'create');
      assert.equal(dry.topics[0].action, 'create');
      const result = JSON.parse(
        await cli([
          'runtime',
          'profiles',
          'restore',
          './synthetic.yaml',
          '--name-prefix',
          prefix,
          '--expect-tsg',
          config.mgmtTsgId,
          '--force',
          '--output',
          'json',
        ]),
      )[0];
      for (const p of result.profiles) if (p.id) ownedProfiles.add(p.id);
      for (const t of result.topics) if (t.action === 'created') ownedTopics.add(t.id);
      assert.equal(result.complete, true);
      assert.equal(result.profiles[0].action, 'created');
      assert.equal(result.topics[0].action, 'created');
      return {
        complete: result.complete,
        createdProfiles: result.profiles.length,
        createdTopics: result.topics.length,
      };
    },
  );
  await check('Collision and no-overwrite protection', async () => {
    const before = await readFile(join(directory, 'synthetic.json'));
    await cli(['runtime', 'profiles', 'backup', name, '--output-file', './synthetic.json'], 1);
    assert.equal(hash(await readFile(join(directory, 'synthetic.json'))), hash(before));
    await cli(
      ['runtime', 'profiles', 'restore', './synthetic.json', '--dry-run', '--name-prefix', prefix],
      1,
    );
  });
} catch (error) {
  results.push({
    check: 'Live workflow',
    passed: false,
    errorType: error?.name,
    statusCode: error?.statusCode,
  });
  console.log(JSON.stringify(results.at(-1)));
  process.exitCode = 1;
} finally {
  // Reconcile names as well as response IDs: a timeout may occur after the service commits.
  try {
    const profiles = await client.profiles.listAll({ latest: true });
    for (const p of profiles)
      if ([name, `${prefix}${name}`].includes(p.profile_name) && p.profile_id)
        ownedProfiles.add(p.profile_id);
    const topics = await client.topics.listAll();
    for (const t of topics)
      if ([name, `${prefix}${name}`].includes(t.topic_name) && t.topic_id)
        ownedTopics.add(t.topic_id);
    for (const id of ownedProfiles) await client.profiles.delete(id);
    for (const id of ownedTopics) await client.topics.delete(id);
    assert.ok(
      !(await client.profiles.listAll({ latest: true })).some((p) =>
        ownedProfiles.has(p.profile_id),
      ),
    );
    assert.ok(!(await client.topics.listAll()).some((t) => ownedTopics.has(t.topic_id)));
    results.push({
      check: 'Only owned synthetic resources removed and absence verified',
      passed: true,
      profiles: ownedProfiles.size,
      topics: ownedTopics.size,
    });
  } catch (error) {
    results.push({
      check: 'Synthetic resource cleanup',
      passed: false,
      statusCode: error?.statusCode,
      ownedProfiles: [...ownedProfiles],
      ownedTopics: [...ownedTopics],
    });
    process.exitCode = 1;
  }
  try {
    await cli(['tenant', 'switch', 'default']);
    await cli(['tenant', 'delete', 'live', '--force']);
    assert.equal(hash(await readFile(source)), hash(bytes));
    results.push({
      check: 'Read-only config hash unchanged; isolated registry restored',
      passed: true,
    });
  } catch {
    results.push({ check: 'Config/registry cleanup', passed: false });
    process.exitCode = 1;
  }
  await writeFile(
    join(directory, 'results.json'),
    `${JSON.stringify({ at: new Date().toISOString(), results }, null, 2)}\n`,
    { flag: 'wx', mode: 0o600 },
  );
  await writeFile(
    join(directory, 'cli-transcript.json'),
    `${JSON.stringify(transcript, null, 2)}\n`,
    { flag: 'wx', mode: 0o600 },
  );
  console.log(
    JSON.stringify({
      artifactDirectory: directory,
      checks: results.length,
      passed: results.filter((r) => r.passed).length,
      results: results.slice(-2),
    }),
  );
}
