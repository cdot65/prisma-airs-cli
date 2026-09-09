// REAL configuration migration. Requires explicit execution and destination identity.
// Never deletes resources or automatically retries writes. Leaves destination selected.
// Run only against an empty destination; partial writes remain if a check fails.
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { mkdtemp, readFile, stat, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs, promisify } from 'node:util';

const { values } = parseArgs({
  options: {
    execute: { type: 'boolean', default: false },
    resume: { type: 'boolean', default: false },
    source: { type: 'string' },
    destination: { type: 'string' },
    'expect-source-tsg': { type: 'string' },
    'expect-tsg': { type: 'string' },
  },
});
assert.ok(
  values.execute &&
    values.source &&
    values.destination &&
    values['expect-source-tsg'] &&
    values['expect-tsg'],
  'Requires --execute --source NAME --destination NAME --expect-source-tsg ID --expect-tsg ID',
);
assert.notEqual(values.source, values.destination);
assert.notEqual(values['expect-source-tsg'], values['expect-tsg']);
const repo = fileURLToPath(new URL('..', import.meta.url));
const entry = process.env.AIRS_CLI_ENTRY ?? join(repo, 'dist/cli/index.js');
await stat(entry);
const directory = await mkdtemp(join(repo, 'artifacts/full-runtime-migration-'));
const env = Object.fromEntries(
  Object.entries(process.env).filter(
    ([key]) =>
      !key.startsWith('PANW_') && !['PRISMA_AIRS_CONFIG_PATH', 'FORCE_COLOR'].includes(key),
  ),
);
Object.assign(env, { DOTENV_CONFIG_PATH: '/dev/null', NO_COLOR: '1' });
const exec = promisify(execFile);
const transcript = [],
  results = [],
  credentialFiles = [];
let secrets = [],
  selectedSource = false;
async function cli(args) {
  const result = await exec(process.execPath, [entry, ...args], {
    cwd: directory,
    env,
    timeout: 300000,
    maxBuffer: 20 * 1024 * 1024,
  }).catch((error) => error);
  assert.ok(
    !secrets.some((secret) => `${result.stdout}${result.stderr}`.includes(secret)),
    'Credential appeared in CLI output',
  );
  transcript.push({
    args,
    exitCode: result.code ?? 0,
    stdout: result.stdout,
    stderr: result.stderr,
  });
  await writeFile(
    join(directory, `command-${transcript.length}.json`),
    JSON.stringify(transcript.at(-1), null, 2),
    { flag: 'wx', mode: 0o600 },
  );
  assert.equal(
    result.code ?? 0,
    0,
    `CLI failed at command ${transcript.length}; inspect private transcript`,
  );
  return result.stdout;
}
const json = async (args) => JSON.parse(await cli([...args, '--output', 'json']));
const inventory = (resource) => json(['runtime', resource, 'list', '--all', '--max', '0']);
function check(name, details = {}) {
  const result = { check: name, passed: true, ...details };
  results.push(result);
  console.log(JSON.stringify(result));
}
const sorted = (items, key) =>
  [...items].sort((a, b) => String(a[key]).localeCompare(String(b[key])));
let initialTenants;
try {
  const help = await cli(['runtime', 'profiles', 'restore', '--help']);
  assert.ok(help.includes('--on-missing-dlp') && help.includes('verify (resume'));
  initialTenants = await json(['tenant', 'list']);
  for (const [name, id] of [
    [values.source, values['expect-source-tsg']],
    [values.destination, values['expect-tsg']],
  ]) {
    const tenant = initialTenants.find((t) => t.name === name);
    assert.equal(tenant?.tsgId, id);
    const bytes = await readFile(tenant.configPath);
    const config = JSON.parse(bytes.toString());
    assert.equal(config.mgmtTsgId, id);
    credentialFiles.push({ path: tenant.configPath, bytes });
    secrets.push(
      ...Object.entries(config)
        .filter(
          ([key, value]) =>
            /secret|token|apikey/i.test(key) && typeof value === 'string' && value.length > 8,
        )
        .map(([, value]) => value),
    );
  }
  await cli(['tenant', 'switch', values.destination]);
  const beforeProfiles = await inventory('profiles'),
    beforeTopics = await inventory('topics');
  if (!values.resume) {
    assert.equal(beforeProfiles.length, 0, 'Destination profiles must be empty');
    assert.equal(beforeTopics.length, 0, 'Destination topics must be empty');
  }
  check('Destination identity and starting inventories checked', {
    profiles: beforeProfiles.length,
    topics: beforeTopics.length,
    resume: values.resume,
  });
  await cli(['tenant', 'switch', values.source]);
  selectedSource = true;
  const sourceProfiles = await inventory('profiles'),
    sourceTopics = await inventory('topics');
  const backupPath = join(directory, 'source-profiles.json');
  const backupResult = (
    await json(['runtime', 'profiles', 'backup', '--all', '--output-file', backupPath])
  )[0];
  const backupBytes = await readFile(backupPath),
    backup = JSON.parse(backupBytes.toString());
  assert.equal(backup.source.tsgId, values['expect-source-tsg']);
  assert.ok(backup.profiles.length > 0);
  assert.equal(backup.profiles.length, sourceProfiles.length);
  assert.equal((await stat(backupPath)).mode & 0o777, 0o600);
  check('Fresh source backup created with private permissions', {
    profiles: backupResult.profiles,
    topics: backupResult.topics,
  });
  await cli(['tenant', 'switch', values.destination]);
  selectedSource = false;
  const args = [
    'runtime',
    'profiles',
    'restore',
    backupPath,
    '--on-missing-dlp',
    'basic',
    '--expect-tsg',
    values['expect-tsg'],
  ];
  if (values.resume) args.push('--on-conflict', 'verify');
  const plan = (await json([...args, '--dry-run']))[0];
  assert.equal(plan.profiles.length, backup.profiles.length);
  assert.ok(
    plan.profiles.every((p) => p.action === 'create' || (values.resume && p.action === 'verify')),
  );
  assert.equal(plan.topics.length, backup.topics.length);
  assert.ok(
    plan.topics.every((t) => t.action === 'create' || (values.resume && t.action === 'reuse')),
  );
  assert.deepEqual(
    sorted(await inventory('profiles'), 'profileId'),
    sorted(beforeProfiles, 'profileId'),
  );
  assert.deepEqual(sorted(await inventory('topics'), 'topicId'), sorted(beforeTopics, 'topicId'));
  check('Dry-run creates nothing; complete dependency plan', {
    profiles: plan.profiles.length,
    topics: plan.topics.length,
    basicFallbacks: plan.dlpFallbacks.length,
  });
  const restored = (await json([...args, '--force']))[0];
  assert.equal(restored.complete, true);
  assert.equal(restored.profiles.length, backup.profiles.length);
  assert.ok(
    restored.profiles.every(
      (p) => p.action === 'created' || (values.resume && p.action === 'verified'),
    ),
  );
  assert.equal(restored.topics.length, backup.topics.length);
  assert.ok(
    restored.topics.every(
      (t) => t.action === 'created' || (values.resume && t.action === 'reused'),
    ),
  );
  check('Full CLI restore and per-resource read-back verification completed', {
    profilesCreated: restored.profiles.filter((p) => p.action === 'created').length,
    profilesVerified: restored.profiles.filter((p) => p.action === 'verified').length,
    topicsCreated: restored.topics.filter((t) => t.action === 'created').length,
    basicFallbacks: restored.dlpFallbacks,
  });
  const profiles = await inventory('profiles'),
    topics = await inventory('topics');
  assert.equal(profiles.length, backup.profiles.length);
  assert.equal(topics.length, backup.topics.length);
  const sourceIds = new Set(backup.topics.map((t) => t.topic_id));
  for (const original of backup.topics) {
    const found = topics.find((t) => t.topicName === original.topic_name);
    assert.ok(found);
    assert.ok(!sourceIds.has(found.topicId));
    assert.equal(found.description, original.description);
    assert.deepEqual(found.examples, original.examples);
  }
  // Independent check of topic bindings, fallback settings, active state and profile identity.
  for (const original of backup.profiles) {
    const found = profiles.find((p) => p.profileName === original.profile_name);
    assert.ok(found);
    assert.notEqual(found.profileId, original.profile_id);
    if (original.active !== undefined) assert.equal(found.active, original.active);
    const effective = structuredClone(original.policy);
    const remap = (value) => {
      if (!value || typeof value !== 'object') return;
      if (value.topic_id) {
        const mapped = topics.find((t) => t.topicName === value.topic_name);
        assert.ok(mapped);
        value.topic_id = mapped.topicId;
        value.revision = mapped.revision;
      }
      Object.values(value).forEach(remap);
    };
    remap(effective);
    if (restored.dlpFallbacks.some((f) => f.profile === original.profile_name)) {
      for (const model of effective['ai-security-profiles'] ?? []) {
        const dlp = model['model-configuration']?.['data-protection']?.['data-leak-detection'];
        if (dlp?.member?.some((m) => m.text !== 'sensitive content' || m.id !== ''))
          dlp.member =
            dlp.action === '' ? null : [{ text: 'sensitive content', id: '', version: '2' }];
      }
      if ('dlp-data-profiles' in effective) effective['dlp-data-profiles'] = [];
    }
    // Separate from the CLI's default allowlist: every explicit source value must survive.
    const preserves = (expected, actual) => {
      if (expected === null || typeof expected !== 'object')
        return assert.deepEqual(actual, expected);
      assert.ok(actual && typeof actual === 'object');
      if (Array.isArray(expected)) {
        assert.ok(Array.isArray(actual));
        assert.equal(actual.length, expected.length);
      }
      for (const [key, value] of Object.entries(expected)) {
        assert.ok(Object.hasOwn(actual, key));
        preserves(value, actual[key]);
      }
    };
    preserves(effective, found.policy);
    const walk = (value) => {
      if (!value || typeof value !== 'object') return;
      if (value.topic_id) {
        const topic = topics.find((t) => t.topicId === value.topic_id);
        assert.ok(topic);
        assert.equal(value.topic_name, topic.topicName);
        assert.equal(value.revision, topic.revision);
      }
      Object.values(value).forEach(walk);
    };
    walk(found.policy);
    if (restored.dlpFallbacks.some((f) => f.profile === original.profile_name)) {
      for (const model of found.policy['ai-security-profiles'] ?? []) {
        const dlp = model['model-configuration']?.['data-protection']?.['data-leak-detection'];
        if (dlp?.member?.length)
          assert.deepEqual(dlp.member, [{ text: 'sensitive content', id: '', version: '2' }]);
      }
    }
  }
  check(
    'Independent inventory, topic definitions/bindings, identity and Basic fallback checks passed',
  );
  const verified = (await json([...args, '--on-conflict', 'verify', '--force']))[0];
  assert.equal(verified.complete, true);
  assert.equal(verified.profiles.length, profiles.length);
  assert.ok(verified.profiles.every((p) => p.action === 'verified'));
  assert.deepEqual(sorted(await inventory('profiles'), 'profileId'), sorted(profiles, 'profileId'));
  assert.deepEqual(sorted(await inventory('topics'), 'topicId'), sorted(topics, 'topicId'));
  const preview = (await json([...args, '--on-conflict', 'verify', '--dry-run']))[0];
  assert.ok(preview.profiles.every((p) => p.action === 'verify'));
  check(
    'Verify rerun and final preview passed; destination IDs, revisions and policies unchanged',
    {
      verified: verified.profiles.length,
    },
  );
  await json([
    'runtime',
    'profiles',
    'backup',
    '--all',
    '--output-file',
    join(directory, 'destination-profiles.json'),
  ]);
  await cli(['tenant', 'switch', values.source]);
  selectedSource = true;
  assert.deepEqual(
    sorted(await inventory('profiles'), 'profileId'),
    sorted(sourceProfiles, 'profileId'),
  );
  assert.deepEqual(sorted(await inventory('topics'), 'topicId'), sorted(sourceTopics, 'topicId'));
  await cli(['tenant', 'switch', values.destination]);
  selectedSource = false;
  assert.deepEqual(await readFile(backupPath), backupBytes);
  check('Source profiles/topics and original backup unchanged; destination backup exported');
} catch (error) {
  results.push({
    check: 'Full migration acceptance',
    passed: false,
    errorType: error.name,
    message: error.name === 'AssertionError' ? error.message.split('\n')[0] : undefined,
  });
  process.exitCode = 1;
} finally {
  if (selectedSource)
    await cli(['tenant', 'switch', values.destination]).catch(() => {
      process.exitCode = 1;
    });
  try {
    for (const file of credentialFiles) assert.deepEqual(await readFile(file.path), file.bytes);
    const finalTenants = await json(['tenant', 'list']);
    assert.equal(finalTenants.find((t) => t.active)?.name, values.destination);
    if (initialTenants)
      assert.deepEqual(
        finalTenants.map(({ active, ...t }) => t),
        initialTenants.map(({ active, ...t }) => t),
      );
    check('Credential files and tenant registrations unchanged; destination selected');
  } catch {
    results.push({ check: 'Final tenant/config verification', passed: false });
    process.exitCode = 1;
  }
  await writeFile(join(directory, 'results.json'), JSON.stringify(results, null, 2), {
    flag: 'wx',
    mode: 0o600,
  });
  console.log(JSON.stringify({ directory, passed: !process.exitCode, checks: results.length }));
}
