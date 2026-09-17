// Dist-tag changes need a package-scoped registry token. Keep the OIDC exchange
// inside the existing trusted publishing workflow; never persist the token.
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

const version = process.env.PROMOTE_VERSION;
const harnessVersion = process.env.HARNESS_VERSION;
assert.match(version ?? '', /^\d+\.\d+\.\d+$/);
assert.match(harnessVersion ?? '', /^\d+\.\d+\.\d+-alpha\.\d+$/);
const manifest = JSON.parse(await readFile(new URL('../package.json', import.meta.url), 'utf8'));
assert.equal(version, manifest.version);
const registry = 'https://registry.npmjs.org';
const name = encodeURIComponent(manifest.name);
async function json(url, options) {
  const response = await fetch(url, { ...options, signal: AbortSignal.timeout(30000) });
  assert.ok(response.ok, `Registry request failed: ${response.status}`);
  return response.json();
}
const published = await json(`${registry}/${name}/${version}`);
assert.deepEqual(published.bin, { 'airs-cli': 'dist/cli/index.js' });
// The harness registry is reachable only from the organization's network.
// Its committed receipt binds anonymous installed acceptance to published bytes.
const acceptance = JSON.parse(
  await readFile(
    new URL(`../validation/command-migration-${version}.json`, import.meta.url),
    'utf8',
  ),
);
assert.equal(acceptance.published, true);
assert.equal(acceptance.cli_version, version);
assert.equal(acceptance.harness_version, harnessVersion);
assert.deepEqual(
  acceptance.installed_platforms.map((item) => `${item.platform}/${item.architecture}`).sort(),
  ['Darwin/arm64', 'Linux/aarch64', 'Linux/x86_64'],
);
assert.ok(
  acceptance.installed_platforms.every((item) => item.passed && item.anonymous_fresh_install),
);
const harness = acceptance.harness_manifest;
assert.equal(harness.version, harnessVersion);
assert.equal(harness.dependencies?.[manifest.name], version);
assert.equal(harness.bin?.airs, 'bin/airs.js');
const requestUrl = new URL(process.env.ACTIONS_ID_TOKEN_REQUEST_URL);
requestUrl.searchParams.set('audience', 'npm:registry.npmjs.org');
const identity = await json(requestUrl, {
  headers: { Authorization: `Bearer ${process.env.ACTIONS_ID_TOKEN_REQUEST_TOKEN}` },
});
const exchanged = await json(`${registry}/-/npm/v1/oidc/token/exchange/package/${name}`, {
  method: 'POST',
  headers: { Authorization: `Bearer ${identity.value}` },
});
assert.ok(exchanged.token);
console.log(`::add-mask::${exchanged.token}`);
const escapedName = manifest.name.replace('/', '%2f');
const response = await fetch(`${registry}/-/package/${escapedName}/dist-tags/latest`, {
  method: 'PUT',
  headers: { Authorization: `Bearer ${exchanged.token}`, 'Content-Type': 'application/json' },
  body: JSON.stringify(version),
  signal: AbortSignal.timeout(30000),
});
if (!response.ok) {
  const detail = (await response.text()).replaceAll(exchanged.token, '[redacted]').slice(0, 500);
  throw new Error(`Promotion failed: ${response.status} ${detail}`);
}
const tags = await json(`${registry}/-/package/${name}/dist-tags`);
assert.equal(tags.latest, version);
console.log(`${manifest.name}@${version} is latest; paired with airs-harness@${harnessVersion}`);
