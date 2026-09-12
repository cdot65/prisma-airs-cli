import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import {
  accessSync,
  constants,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import { dirname, isAbsolute, join, resolve } from 'node:path';
import { nativeDiagnostics } from './native-diagnostics.mjs';

// Test the BUILT/INSTALLED CLI, not an imported command or mocked generator.
// No credentials, inherited runtime configuration, external API or persistent writes.
const entry = process.argv[2];
assert.ok(entry && isAbsolute(entry), 'Pass an absolute built/installed CLI entry point.');
accessSync(entry, constants.R_OK);
const packageRoot = resolve(dirname(entry), '../..');
const pkg = JSON.parse(readFileSync(join(packageRoot, 'package.json'), 'utf8'));
const consumerRequire = createRequire(entry);
const sharp = consumerRequire('sharp');
assert.ok(sharp.versions.vips, 'Native libvips must load from this consumer.');
const work = mkdtempSync(join(tmpdir(), 'prisma-airs-dlp-consumer-'));
const config = join(work, 'config.json');
const registry = join(work, 'tenants.json');
const cache = join(work, 'cache');
mkdirSync(cache);
// Configuration comes only from the selected tenant file; the registry below selects it.
function writeTenant(settings = {}) {
  const credentials = {
    mgmtClientId: 'consumer',
    mgmtClientSecret: 'consumer-secret',
    mgmtTsgId: '1',
  };
  writeFileSync(config, `${JSON.stringify({ ...credentials, ...settings })}\n`, { mode: 0o600 });
  writeFileSync(
    registry,
    `${JSON.stringify({ version: 1, active: 'consumer', tenants: [{ name: 'consumer', configPath: config, tsgId: '1' }] })}\n`,
    { mode: 0o600 },
  );
}
const env = {
  PATH: process.env.PATH,
  PRISMA_AIRS_TENANTS_PATH: registry,
  NO_COLOR: '1',
  // Give fontconfig a writable, disposable cache without inheriting the user's home.
  XDG_CACHE_HOME: cache,
  ...(process.env.FONTCONFIG_FILE ? { FONTCONFIG_FILE: process.env.FONTCONFIG_FILE } : {}),
};
const results = [];
const runtimeWarnings = new Set();

function run(args, overrides = {}) {
  const result = spawnSync(process.execPath, [entry, ...args], {
    cwd: work,
    env: { ...env, ...overrides },
    encoding: 'utf8',
    timeout: 30000,
    maxBuffer: 1024 * 1024,
  });
  assert.ifError(result.error);
  assert.equal(result.signal, null);
  if (result.status === 0) {
    for (const warning of nativeDiagnostics(result.stderr, process.version))
      runtimeWarnings.add(warning);
  }
  return result;
}

function caseResult(name, fn) {
  fn();
  results.push({ name, passed: true });
}

function checkSignature(path, format) {
  const bytes = readFileSync(path);
  if (format === 'pdf') assert.equal(bytes.subarray(0, 5).toString(), '%PDF-');
  if (format === 'png') assert.equal(bytes.subarray(0, 8).toString('hex'), '89504e470d0a1a0a');
  if (format === 'jpeg') assert.equal(bytes.subarray(0, 3).toString('hex'), 'ffd8ff');
  if (format === 'docx') assert.equal(bytes.subarray(0, 4).toString('hex'), '504b0304');
  if (format === 'svg') assert.match(bytes.toString(), /<svg[\s>]/);
}

try {
  writeTenant();
  caseResult('installed CLI version matches its package', () => {
    const result = run(['--version']);
    assert.equal(result.status, 0);
    assert.equal(result.stdout.trim(), pkg.version);
  });
  const out = join(work, 'all-formats');
  let summary;
  caseResult(
    'all five native formats: JSON stdout, 5 clean + 21 dirty, manifest and 26 signatures',
    () => {
      const result = run([
        'runtime',
        'dlp',
        'generate',
        '--types',
        'all',
        '--count',
        '1',
        '--seed',
        '431',
        '--out',
        out,
        '--output',
        'json',
      ]);
      assert.equal(result.status, 0, result.stderr);
      summary = JSON.parse(result.stdout);
      assert.equal(summary.seed, 431);
      assert.equal(summary.clean, 5);
      assert.equal(summary.dirty, 21);
      const manifest = JSON.parse(readFileSync(summary.manifestPath, 'utf8'));
      assert.equal(manifest.count, 21);
      assert.equal(manifest.entries.length, 21);
      const checked = new Set();
      for (const item of manifest.entries) {
        for (const path of [item.clean, item.dirty]) {
          assert.ok(path.startsWith(`${out}/`));
          checkSignature(path, item.format);
          checked.add(path);
        }
      }
      assert.equal(checked.size, 26);
    },
  );
  caseResult('global JSON and quiet preserve structured output', () => {
    const result = run([
      '--output',
      'json',
      '--quiet',
      'runtime',
      'dlp',
      'generate',
      '--types',
      'svg',
      '--seed',
      '431',
      '--out',
      join(work, 'global'),
    ]);
    assert.equal(result.status, 0, result.stderr);
    assert.equal(JSON.parse(result.stdout).clean, 1);
  });
  caseResult('file default JSON is respected', () => {
    writeTenant({ defaultOutput: 'json' });
    const result = run([
      'runtime',
      'dlp',
      'generate',
      '--types',
      'svg',
      '--out',
      join(work, 'configured'),
    ]);
    assert.equal(result.status, 0, result.stderr);
    assert.equal(JSON.parse(result.stdout).dirty, 4);
  });
  caseResult('environment variables are ignored; the tenant file pretty default wins', () => {
    writeTenant({ defaultOutput: 'pretty' });
    const result = run(
      ['runtime', 'dlp', 'generate', '--types', 'svg', '--out', join(work, 'environment')],
      { PANW_CLI_OUTPUT: 'json' },
    );
    assert.equal(result.status, 0, result.stderr);
    assert.throws(() => JSON.parse(result.stdout));
    assert.match(result.stdout, /svg/);
  });
  caseResult(
    'explicit pretty overrides environment JSON and quiet preserves per-format data',
    () => {
      const result = run(
        [
          '--quiet',
          'runtime',
          'dlp',
          'generate',
          '--types',
          'svg',
          '--out',
          join(work, 'pretty'),
          '--output',
          'pretty',
        ],
        { PANW_CLI_OUTPUT: 'json' },
      );
      assert.equal(result.status, 0, result.stderr);
      assert.match(result.stdout, /svg/);
      assert.doesNotMatch(result.stdout, /DLP Test-File Generation/);
    },
  );
  for (const args of [
    ['--output', 'yaml'],
    ['--output', 'invalid'],
    ['--count', '7junk'],
    ['--seed', 'NaN'],
    ['--seed', '1.5'],
  ]) {
    caseResult(`invalid ${args.join(' ')} exits 2 before filesystem mutation`, () => {
      const destination = join(work, `invalid-${results.length}`);
      const result = run(['runtime', 'dlp', 'generate', '--out', destination, ...args]);
      assert.equal(result.status, 2);
      assert.equal(result.stdout, '');
      assert.ok(result.stderr.length > 0);
      assert.equal(existsSync(destination), false);
    });
  }
  const report = {
    generatedAt: new Date().toISOString(),
    cliVersion: pkg.version,
    sdkVersion: pkg.dependencies['@cdot65/prisma-airs-sdk'],
    nodeVersion: process.version,
    sharpVersion: sharp.versions.sharp,
    libvipsVersion: sharp.versions.vips,
    customFontConfiguration: Boolean(process.env.FONTCONFIG_FILE),
    runtimeWarnings: [...runtimeWarnings],
    passed: true,
    cases: results,
    fixtureCleanup: 'complete',
    summary: {
      ...summary,
      out: '<temporary-corpus>',
      manifestPath: '<temporary-corpus>/manifest.json',
    },
  };
  // Cleanup must succeed before the report can certify it.
  rmSync(work, { recursive: true });
  const json = `${JSON.stringify(report, null, 2)}\n`;
  if (process.argv[3]) writeFileSync(process.argv[3], json);
  process.stdout.write(json);
} finally {
  if (existsSync(work)) rmSync(work, { recursive: true });
}
