import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { isAbsolute, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { nativeDiagnostics } from './native-diagnostics.mjs';

const root = fileURLToPath(new URL('../../', import.meta.url));
const [installed, archive] = process.argv.slice(2);
assert.ok(installed && isAbsolute(installed), 'Pass an absolute installed package directory.');
assert.ok(archive && isAbsolute(archive), 'Pass an absolute independently packed tarball.');
const expected = JSON.parse(readFileSync(resolve(root, 'package.json'), 'utf8'));
const files = execFileSync('tar', ['-tzf', archive], { encoding: 'utf8' }).trim().split('\n');
assert.equal(new Set(files).size, files.length);
assert.ok(files.includes('package/dist/cli/index.js'));
assert.ok(files.includes('package/dist/index.js'));
assert.ok(files.includes('package/dist/index.d.ts'));
assert.ok(files.includes('package/package.json'));
assert.ok(files.includes('package/README.md'));
for (const packed of files) {
  assert.match(
    packed,
    /^package\/(?:dist\/[a-zA-Z0-9_./-]+|package\.json|README\.md|LICENSE(?:\.\w+)?)$/,
  );
  assert.ok(!packed.split('/').includes('..'), 'Unsafe archive path.');
  const file = packed.slice('package/'.length);
  const source = readFileSync(resolve(root, file));
  assert.ok(
    source.equals(readFileSync(resolve(installed, file))),
    `Installed payload differs: ${file}`,
  );
  assert.ok(
    source.equals(execFileSync('tar', ['-xOf', archive, packed], { maxBuffer: 8_000_000 })),
    `Packed payload differs: ${file}`,
  );
}
const pkg = JSON.parse(readFileSync(resolve(installed, 'package.json'), 'utf8'));
assert.equal(pkg.version, expected.version);
assert.equal(
  pkg.dependencies['@cdot65/prisma-airs-sdk'],
  expected.dependencies['@cdot65/prisma-airs-sdk'],
);
const sdk = createRequire(resolve(installed, 'package.json'))('@cdot65/prisma-airs-sdk');
assert.equal(sdk.SDK_VERSION, expected.dependencies['@cdot65/prisma-airs-sdk']);
const entry = resolve(installed, 'dist/cli/index.js');
const diagnostics = new Set();
const command = (...args) => {
  const result = spawnSync(process.execPath, [entry, ...args], {
    encoding: 'utf8',
    timeout: 30000,
    maxBuffer: 1048576,
    env: { PATH: process.env.PATH, NO_COLOR: '1' },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  assert.ifError(result.error);
  assert.equal(result.signal, null);
  assert.equal(result.status, 0);
  for (const warning of nativeDiagnostics(result.stderr, process.version)) diagnostics.add(warning);
  return result.stdout;
};
assert.equal(command('--version').trim(), expected.version);
const help = command('aigateway', 'inference', '--help');
for (const name of ['chat', 'responses', 'embeddings']) assert.ok(help.includes(name));
const library = await import(pathToFileURL(resolve(installed, 'dist/index.js')).href);
const localLibrary = await import(pathToFileURL(resolve(root, 'dist/index.js')).href);
assert.deepEqual(Object.keys(library).sort(), Object.keys(localLibrary).sort());
for (const name of [
  'collectRuntimeDailyReport',
  'renderRuntimeReportHtml',
  'renderRuntimeReportMarkdown',
  'writeReportFile',
])
  assert.equal(typeof library[name], 'function');
const reportHelp = command('runtime', 'report', '--help');
for (const flag of ['--output', '--output-file', '--title', '--max-pages', '--strict'])
  assert.ok(reportHelp.includes(flag));
const dashboardHelp = command('runtime', 'dashboard', '--help');
for (const name of [
  'applications',
  'application-violations',
  'top-applications',
  'violations-trend',
  'apps-list',
])
  assert.ok(dashboardHelp.includes(name));
const sessionsHelp = command('runtime', 'sessions', '--help');
for (const name of ['chart', 'list', 'get', 'transaction', 'scan-content'])
  assert.ok(sessionsHelp.includes(name));
const contentHelp = command('runtime', 'sessions', 'scan-content', '--help');
for (const flag of ['--scan-id', '--scan-sub-req-id', '--show-content', '--output-file'])
  assert.ok(contentHelp.includes(flag));
for (const metric of ['requests', 'cost', 'tokens', 'latency', 'group-by']) {
  const flags = command('aigateway', 'telemetry', metric, '--help');
  for (const flag of [
    '--trace-id',
    '--metadata',
    '--status-codes',
    '--api-key-ids',
    '--ai-org-models',
    '--total-units-min',
    '--total-units-max',
    '--cost-min',
    '--cost-max',
  ])
    assert.ok(flags.includes(flag));
  if (metric === 'group-by') {
    for (const dimension of ['ai_service', 'model', 'api_key', 'provider', 'status_code', 'users'])
      assert.ok(flags.includes(dimension));
    assert.ok(flags.includes('--columns'));
  }
}
const bytes = readFileSync(archive);
console.log(
  JSON.stringify(
    {
      checkedAt: new Date().toISOString(),
      installed,
      archive,
      bytes: bytes.byteLength,
      sha256: createHash('sha256').update(bytes).digest('hex'),
      version: pkg.version,
      sdkVersion: sdk.SDK_VERSION,
      verifiedPayloadFiles: files.length,
      distributionFiles: files.filter((file) => file.startsWith('package/dist/')).length,
      libraryExports: Object.keys(library).length,
      packedAndInstalledPayloadsIdentical: true,
      versionAndInferenceHelpPassed: true,
      groupFilterHelpPassed: true,
      runtimeReportHelpPassed: true,
      runtimeDashboardAndSessionHelpPassed: true,
      runtimeWarnings: [...diagnostics],
      passed: true,
    },
    null,
    2,
  ),
);
