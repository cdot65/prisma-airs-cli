// Read-only live acceptance. Run after installing dependencies and building this CLI.
// Never executes browser curl captures or reuses their bearer tokens.
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdir, mkdtemp, readdir, readFile, stat } from 'node:fs/promises';
import { homedir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';
import * as sdk from '@cdot65/prisma-airs-sdk';
import { load as yaml } from 'js-yaml';

const exec = promisify(execFile);
process.env.PANW_AI_SEC_DEBUG = '0';
process.env.PANW_AI_SEC_DEBUG_BODY = '0';
const repo = fileURLToPath(new URL('..', import.meta.url));
const configPath =
  process.env.PRISMA_AIRS_CONFIG_PATH ?? join(homedir(), '.prisma-airs/config.json');
const configBytes = await readFile(configPath);
const config = JSON.parse(configBytes.toString());
const digest = (bytes) => createHash('sha256').update(bytes).digest('hex');
const env = {
  ...process.env,
  PRISMA_AIRS_CONFIG_PATH: configPath,
  PANW_AI_SEC_DEBUG: '0',
  PANW_AI_SEC_DEBUG_BODY: '0',
  NO_COLOR: '1',
};
const credentials = {
  clientId: config.mgmtClientId,
  clientSecret: config.mgmtClientSecret,
  tsgId: config.mgmtTsgId,
  tokenEndpoint: config.mgmtTokenEndpoint,
  numRetries: 0,
};
const agent = new sdk.AgentGuardClient({
  ...credentials,
  dataEndpoint: config.agentGuardDataEndpoint,
  mgmtEndpoint: config.agentGuardMgmtEndpoint,
});
const model = new sdk.ModelSecurityClient({
  ...credentials,
  dataEndpoint: 'https://api.apps.paloaltonetworks.com/aims/data',
  mgmtEndpoint: 'https://api.apps.paloaltonetworks.com/aims/mgmt',
});
const results = [];
async function check(name, run) {
  try {
    const evidence = await run();
    results.push({ name, passed: true, ...evidence });
    console.log(JSON.stringify(results.at(-1)));
  } catch (error) {
    results.push({
      name,
      passed: false,
      statusCode: Number.isInteger(error?.statusCode) ? error.statusCode : undefined,
    });
    console.log(JSON.stringify(results.at(-1)));
  }
}
if (process.env.AIRS_BROWSER_CAPTURE_PATH)
  await check('Nine captured response contracts', async () => {
    const text = await readFile(process.env.AIRS_BROWSER_CAPTURE_PATH, 'utf8');
    const blocks = text.split(/(?=^curl )/m).filter((b) => b.startsWith('curl '));
    const schemas = [
      sdk.ModelListSchema,
      sdk.ScanListSchema,
      sdk.ScanListSchema,
      sdk.ListModelSecurityGroupsResponseSchema,
      sdk.ModelVersionResponseSchema,
      sdk.AgentGuardScanListSchema,
      sdk.AgentGuardScanStatsSchema,
      sdk.AgentGuardVulnerabilityListSchema,
      sdk.AgentGuardRuleListSchema,
    ];
    assert.equal(blocks.length, schemas.length);
    for (const [i, block] of blocks.entries()) {
      const start = block.search(/^\s*[[{]/m);
      let body;
      for (let end = block.length; end > start; end = block.lastIndexOf('\n', end - 1)) {
        try {
          body = JSON.parse(block.slice(start, end).trim());
          break;
        } catch {}
      }
      schemas[i].parse(body);
    }
    return { validated: schemas.length };
  });
const end = new Date();
const start = new Date(end.getTime() - 30 * 86400000);
const window = { start_time: start.toISOString(), end_time: end.toISOString() };
let version;
let findingScan;
await check('SDK models + fresh OAuth', async () => {
  const p = await model.models.listModels({
    skip: 0,
    limit: 15,
    isBackgroundRefresh: false,
    ...window,
  });
  version = p.models[0]?.latest_version_uuid;
  assert.ok(version);
  return { returned: p.models.length, total: p.pagination.total_items };
});
await check('SDK scans by model version', async () => {
  assert.ok(version);
  const p = await model.scans.list({ model_version_uuid: version, limit: 1 });
  return { returned: p.scans.length, total: p.pagination.total_items };
});
await check('SDK model scans', async () => {
  const p = await model.scans.list({ skip: 0, limit: 15, isBackgroundRefresh: false });
  return { returned: p.scans.length, total: p.pagination.total_items };
});
await check('SDK model security groups', async () => {
  const p = await model.securityGroups.list({ limit: 30 });
  return { returned: p.security_groups.length, total: p.pagination.total_items };
});
await check('SDK model version', async () => {
  assert.ok(version);
  const p = await model.models.getModelVersion(version);
  assert.equal(p.uuid, version);
  return { identityMatched: true };
});
await check('SDK AgentGuard scans + fresh OAuth', async () => {
  const p = await agent.listScans({ skip: 0, limit: 10, isBackgroundRefresh: false, ...window });
  findingScan = p.scans.find((s) => (s.summary?.vulnerability_count ?? 0) > 0 && !s.is_batch)?.uuid;
  assert.ok(findingScan);
  return { returned: p.scans.length, total: p.pagination.total_items };
});
await check('SDK AgentGuard statistics', async () => {
  const p = await agent.getScanStats();
  return {
    uniqueSkills: p.unique_skills_scanned.count,
    vulnerabilities: p.total_vulnerabilities_found.count,
  };
});
await check('SDK AgentGuard vulnerabilities', async () => {
  assert.ok(findingScan);
  const p = await agent.listScanVulnerabilities(findingScan);
  assert.ok(p.vulnerabilities.length);
  return { returned: p.vulnerabilities.length, total: p.pagination.total_items };
});
await check('SDK AgentGuard rules', async () => {
  const p = await agent.listRules({ skip: 0, limit: 100 });
  return { returned: p.rules.length, total: p.pagination.total_items };
});

await mkdir(resolve(repo, 'artifacts'), { recursive: true });
const directory = await mkdtemp(resolve(repo, 'artifacts/agentguard-e2e-'));
const cli = async (args) =>
  exec(
    process.execPath,
    [resolve(process.env.AIRS_CLI_ENTRY ?? resolve(repo, 'dist/cli/index.js')), '--quiet', ...args],
    {
      cwd: directory,
      env,
      timeout: 90000,
      maxBuffer: 8 * 1024 * 1024,
    },
  );
for (const format of ['json', 'yaml', 'pretty', 'table', 'markdown', 'csv'])
  await check(`CLI scans ${format}`, async () => {
    const r = await cli(['agentguard', 'scans', 'list', '--output', format]);
    assert.ok(r.stdout.trim());
    if (format === 'json' || format === 'yaml') {
      const p = format === 'json' ? JSON.parse(r.stdout) : yaml(r.stdout);
      assert.ok(p.scans.length);
      for (const row of p.scans)
        for (const field of ['name', 'git_url', 'device_metadata', 'eval_details', 'fingerprint'])
          assert.equal(field in row, false);
    }
    return { exitCode: 0 };
  });
await check('CLI all scan pages', async () => {
  const r = await cli(['agentguard', 'scans', 'list', '--all', '--limit', '3', '--output', 'json']);
  const p = JSON.parse(r.stdout);
  assert.equal(p.pagination.truncated, false);
  assert.equal(p.scans.length, p.pagination.total_items);
  return { returned: p.scans.length, total: p.pagination.total_items };
});
await check('CLI findings safe metadata', async () => {
  assert.ok(findingScan);
  const r = await cli(['agentguard', 'scans', 'vulnerabilities', findingScan, '--output', 'json']);
  const p = JSON.parse(r.stdout);
  assert.ok(p.vulnerabilities.length);
  assert.equal(p.content_included, false);
  for (const row of p.vulnerabilities)
    for (const field of ['file_path', 'description', 'original_code', 'suggested_code'])
      assert.equal(field in row, false);
  return { returned: p.vulnerabilities.length };
});
await check('CLI statistics actual output', async () => {
  const r = await cli(['agentguard', 'stats', '--output', 'json']);
  const p = sdk.AgentGuardScanStatsSchema.parse(JSON.parse(r.stdout));
  return { output: p };
});
await check('CLI all rule pages', async () => {
  const r = await cli(['agentguard', 'rules', 'list', '--all', '--limit', '3', '--output', 'yaml']);
  const p = yaml(r.stdout);
  const expected = await agent.listRules({ limit: 100 });
  assert.equal(p.pagination.truncated, false);
  assert.equal(p.rules.length, p.pagination.total_items);
  assert.deepEqual(p.rules.map((x) => x.uuid).sort(), expected.rules.map((x) => x.uuid).sort());
  return { returned: p.rules.length };
});
for (const format of ['html', 'markdown'])
  await check(`CLI strict ${format} report`, async () => {
    await cli(['agentguard', 'report', '--strict', '--output', format]);
    const extension = format === 'html' ? '.html' : '.md';
    const name = (await readdir(directory)).find((n) => n.endsWith(extension));
    assert.ok(name);
    const path = join(directory, name);
    const text = await readFile(path, 'utf8');
    assert.equal((await stat(path)).mode & 0o777, 0o600);
    assert.ok(text.startsWith(format === 'html' ? '<!doctype html>' : '# AgentGuard'));
    assert.ok(text.includes('Skill statistics'));
    assert.equal(text.includes('PRIVATE-'), false);
    return { file: path, bytes: Buffer.byteLength(text) };
  });
await check('CLI stdout Markdown report', async () => {
  const r = await cli([
    'agentguard',
    'report',
    '--strict',
    '--output',
    'markdown',
    '--output-file',
    '-',
  ]);
  assert.ok(r.stdout.startsWith('# AgentGuard'));
  assert.ok(!r.stdout.includes('Report →'));
  return { bytes: Buffer.byteLength(r.stdout) };
});
await check('CLI report no overwrite', async () => {
  const path = join(
    directory,
    (await readdir(directory)).find((n) => n.endsWith('.html')),
  );
  const before = await readFile(path);
  await assert.rejects(cli(['agentguard', 'report', '--output-file', path]), (e) => e.code === 1);
  assert.equal(digest(await readFile(path)), digest(before));
  return { unchanged: true };
});
await check('CLI Model Security filtered reads', async () => {
  const first = await cli([
    'model-security',
    'models',
    'list',
    '--start',
    window.start_time,
    '--end',
    window.end_time,
    '--output',
    'json',
  ]);
  assert.ok(Array.isArray(JSON.parse(first.stdout)));
  assert.ok(version);
  const second = await cli([
    'model-security',
    'scans',
    'list',
    '--model-version',
    version,
    '--limit',
    '1',
    '--output',
    'json',
  ]);
  assert.equal(JSON.parse(second.stdout).length, 1);
  return { commands: 2 };
});
await check('Credentials file unchanged', async () => {
  assert.equal(digest(await readFile(configPath)), digest(configBytes));
  return { unchanged: true };
});
console.log(
  JSON.stringify({
    summary: {
      passed: results.filter((r) => r.passed).length,
      failed: results.filter((r) => !r.passed).length,
    },
    artifacts: directory,
  }),
);
if (results.some((r) => !r.passed)) process.exitCode = 1;
