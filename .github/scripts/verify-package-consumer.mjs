import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { isAbsolute, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

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
const command = (...args) =>
  execFileSync(process.execPath, [entry, ...args], { encoding: 'utf8', timeout: 30000 });
assert.equal(command('--version').trim(), expected.version);
const help = command('aigateway', 'inference', '--help');
for (const name of ['chat', 'responses', 'embeddings']) assert.ok(help.includes(name));
const library = await import(pathToFileURL(resolve(installed, 'dist/index.js')).href);
assert.equal(Object.keys(library).length, 19);
for (const metric of ['requests', 'cost', 'tokens', 'latency']) {
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
}
console.log(
  JSON.stringify(
    {
      checkedAt: new Date().toISOString(),
      version: pkg.version,
      sdkVersion: sdk.SDK_VERSION,
      verifiedPayloadFiles: files.length,
      distributionFiles: files.filter((file) => file.startsWith('package/dist/')).length,
      libraryExports: Object.keys(library).length,
      packedAndInstalledPayloadsIdentical: true,
      versionAndInferenceHelpPassed: true,
      passed: true,
    },
    null,
    2,
  ),
);
