import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { AdvancedDataProfileRequestSchema } from '@cdot65/prisma-airs-sdk';
import { buildProfileRequest } from '../../../src/cli/builders/profile-builder.js';

const helper = resolve('scripts/runtime-migration-helpers.bash');
const document = readFileSync('docs-site/docs/runtime/prod-dev-migration.md', 'utf8');
let directory: string;
beforeEach(() => {
  directory = mkdtempSync(join(tmpdir(), 'airs-guide-helper-'));
});
afterEach(() => {
  rmSync(directory, { recursive: true, force: true });
});

function bash(script: string) {
  return spawnSync(
    'bash',
    ['-c', `set -e\nairs() { return 99; }\nsource "$1"\n${script}`, 'helper-test', helper],
    {
      cwd: directory,
      encoding: 'utf8',
      timeout: 10000,
      env: { PATH: process.env.PATH, NODE_BIN: process.execPath },
    },
  );
}

it('syntax-checks direct CLI examples and internal capture tooling separately', () => {
  const scripts = [...document.matchAll(/```bash\n([\s\S]*?)```/g)].map((m) => m[1]);
  expect(scripts.length).toBeGreaterThanOrEqual(12);
  scripts.push(readFileSync(helper, 'utf8'));
  for (const script of scripts) {
    const parsed = spawnSync('bash', ['-n'], { input: script, encoding: 'utf8' });
    expect(parsed.status, parsed.stderr).toBe(0);
  }
});

function commands() {
  const scripts = [...document.matchAll(/```bash\n([\s\S]*?)```/g)].map((m) => m[1]);
  // Only a shell builtin records argv. Never invoke the real CLI or cloud writes.
  return scripts.flatMap((script) => {
    const result = spawnSync(
      '/bin/bash',
      ['-c', `airs() { printf '%s\\0' "$@"; printf '\\036'; }\n${script}`],
      {
        cwd: directory,
        encoding: 'utf8',
        timeout: 1000,
        env: { PATH: '' },
      },
    );
    expect(result.status, result.stderr).toBe(0);
    return result.stdout
      .split('\x1e')
      .filter(Boolean)
      .map((line) => line.split('\0').slice(0, -1));
  });
}

it('passes valid explicit DLP rule trees with distinct tenant placeholders', () => {
  const requests = commands().filter((args) => args.includes('--body'));
  expect(requests).toHaveLength(2);
  for (const [index, args] of requests.entries()) {
    const body = JSON.parse(args[args.indexOf('--body') + 1]);
    expect(() => AdvancedDataProfileRequestSchema.parse(body)).not.toThrow();
    expect(body.detection_rules[0].expression_tree.sub_expressions[0].rule_item).toEqual({
      detection_technique: 'regex',
      id: index === 0 ? '<PROD_PATTERN_ID>' : '<DEV_PATTERN_ID>',
      name: 'dlp-test-pattern',
      match_type: 'include',
      confidence_level: 'high',
      occurrence_operator_type: 'any',
      occurrence_count: 1,
    });
  }
});

it('builds the three documented Runtime protection selections from real CLI flags', () => {
  const creates = commands().filter(
    (args) => args.slice(0, 3).join(' ') === 'runtime profiles create',
  );
  expect(creates).toHaveLength(3);
  const policies = creates.map((args) => {
    const option = (flag: string) =>
      args.includes(flag) ? args[args.indexOf(flag) + 1] : undefined;
    return buildProfileRequest({
      name: option('--name') ?? '',
      dlpAction: option('--dlp-action'),
      dlpProfiles: option('--dlp-profiles'),
      inlineTimeoutAction: option('--inline-timeout-action'),
      maxInlineLatency: option('--max-inline-latency')
        ? Number(option('--max-inline-latency'))
        : undefined,
    }).policy?.['ai-security-profiles']?.[0]?.['model-configuration']?.['data-protection']?.[
      'data-leak-detection'
    ];
  });
  expect(policies[0]).toMatchObject({ action: 'block', member: [{ text: 'dlp-test' }] });
  expect(policies[1]).toMatchObject({ action: 'block', member: [{ text: 'sensitive content' }] });
  expect(policies[2]).toMatchObject({ action: '', member: null });
});

it('sourcing saved helpers makes no CLI calls and exports no stale tenant IDs', () => {
  const result = bash(
    'declare -F capture json_id assert_empty load_tenant_ids create_test_dlp; test -z "$PROD_TSG"; test -z "$DEV_TSG"',
  );
  expect(result.status, result.stderr).toBe(0);
});

it('reloads and exports tenant IDs without needing old shell state', () => {
  const result =
    bash(`airs() { printf '%s' '[{"name":"prod","tsgId":"100"},{"name":"dev","tsgId":"200"}]'; }
load_tenant_ids && "$NODE_BIN" -e 'if(process.env.PROD_TSG!=="100"||process.env.DEV_TSG!=="200")process.exit(1)'`);
  expect(result.status, result.stderr).toBe(0);
});

it('refuses incomplete registrations', () => {
  const result = bash(`airs() { printf '%s' '[{"name":"prod","tsgId":"100"}]'; }
load_tenant_ids`);
  expect(result.status).not.toBe(0);
});

it('captures stdout, stderr and failure status without inventing a success', () => {
  const result = bash(`command_fixture() { printf 'output'; printf 'diagnostic' >&2; return 7; }
capture result.txt command_fixture`);
  expect(result.status).toBe(7);
  expect(readFileSync(join(directory, 'result.txt'), 'utf8')).toBe('output');
  expect(readFileSync(join(directory, 'result.txt.stderr'), 'utf8')).toBe('diagnostic');
  expect(readFileSync(join(directory, 'result.txt.exit-code.txt'), 'utf8')).toBe('7\n');
  expect(readFileSync(join(directory, 'result.txt.command.txt'), 'utf8')).toContain(
    'command_fixture',
  );
});

it.each([
  'result.txt',
  'result.txt.command.txt',
  'result.txt.stderr',
  'result.txt.exit-code.txt',
])('does not invoke a command when %s already exists', (file) => {
  writeFileSync(join(directory, file), 'keep-original');
  const result = bash(`command_fixture() { printf 'should-not-run'; }
capture result.txt command_fixture`);
  expect(result.status).toBe(1);
  expect(result.stdout).not.toContain('should-not-run');
  expect(readFileSync(join(directory, file), 'utf8')).toBe('keep-original');
});
