import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { Script } from 'node:vm';

const helper = resolve('docs-site/static/examples/runtime-migration-helpers.bash');
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

it('syntax-checks all documented Bash blocks and embedded Node checks', () => {
  const scripts = [...document.matchAll(/```bash\n([\s\S]*?)```/g)].map((m) => m[1]);
  expect(scripts.length).toBeGreaterThanOrEqual(12);
  scripts.push(readFileSync(helper, 'utf8'));
  for (const script of scripts) {
    const parsed = spawnSync('bash', ['-n'], { input: script, encoding: 'utf8' });
    expect(parsed.status, parsed.stderr).toBe(0);
    for (const node of script.matchAll(/<<'NODE'\n([\s\S]*?)\nNODE/g))
      expect(() => new Script(node[1])).not.toThrow();
  }
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
