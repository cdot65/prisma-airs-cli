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
const harness = await json(`https://npm.cdot.io/airs-harness/${harnessVersion}`);
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
const response = await fetch(`${registry}/-/package/${name}/dist-tags/latest`, {
  method: 'PUT',
  headers: { Authorization: `Bearer ${exchanged.token}`, 'Content-Type': 'application/json' },
  body: JSON.stringify(version),
  signal: AbortSignal.timeout(30000),
});
assert.ok(response.ok, `Promotion failed: ${response.status}`);
const tags = await json(`${registry}/-/package/${name}/dist-tags`);
assert.equal(tags.latest, version);
console.log(`${manifest.name}@${version} is latest; paired with airs-harness@${harnessVersion}`);
