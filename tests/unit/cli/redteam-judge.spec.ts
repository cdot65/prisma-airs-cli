import { mkdtemp, readdir, readFile, rm, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Command } from 'commander';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  JUDGE_PROVIDER_ERROR_EXIT_CODE,
  parsePolicy,
  registerRedTeamJudgeCommand,
} from '../../../src/cli/commands/redteam-judge.js';
import { buildProgram } from '../../../src/cli/program.js';

const SCAN = new URL('../../fixtures/redteam-judge/sample-scan.json', import.meta.url).pathname;
const JUDGMENTS = new URL('../../fixtures/redteam-judge/sample-judgments.json', import.meta.url)
  .pathname;

const mock = vi.hoisted(() => ({
  config: {} as Record<string, unknown>,
  clientOptions: vi.fn(),
  listAttacks: vi.fn(),
  getAttackDetail: vi.fn(),
}));
vi.mock('@cdot65/prisma-airs-sdk', async (original) => ({
  ...(await original<typeof import('@cdot65/prisma-airs-sdk')>()),
  RedTeamClient: class {
    reports = { listAttacks: mock.listAttacks, getAttackDetail: mock.getAttackDetail };
    constructor(options: unknown) {
      mock.clientOptions(options);
    }
  },
}));
vi.mock('../../../src/config/loader.js', () => ({
  loadConfig: vi.fn(async () => {
    if (mock.config.__throw) throw new Error('No tenant selected');
    return { scanConcurrency: 5, dataDir: '/tmp', ...mock.config };
  }),
  resolveConfigContext: vi.fn(() => ({
    selection: 'tenant',
    path: '/cfg/dev.json',
    tenant: { name: 'dev', configPath: '/cfg/dev.json', tsgId: '100' },
    registryPath: '/state/tenants.json',
  })),
}));

const MGMT = { mgmtClientId: 'c', mgmtClientSecret: 's', mgmtTsgId: '100' };
let logs: string[];
let errors: string[];
let directory: string;

function program() {
  const root = new Command().exitOverride();
  const redteam = root.command('redteam').exitOverride();
  registerRedTeamJudgeCommand(redteam);
  return root;
}
async function run(args: string[]) {
  await program().parseAsync(['redteam', 'judge', ...args], { from: 'user' });
}
const stdout = () => logs.join('\n');

beforeEach(async () => {
  vi.clearAllMocks();
  mock.config = { typesafeApiKey: 'test-only-typesafe-key' };
  logs = [];
  errors = [];
  directory = await mkdtemp(join(tmpdir(), 'airs-judge-test-'));
  vi.spyOn(console, 'log').mockImplementation((...args: unknown[]) => {
    logs.push(args.join(' '));
  });
  vi.spyOn(console, 'error').mockImplementation((...args: unknown[]) => {
    errors.push(args.join(' '));
  });
  vi.spyOn(process, 'exit').mockImplementation((code) => {
    throw new Error(`EXIT:${code}`);
  });
});
afterEach(async () => {
  vi.restoreAllMocks();
  process.exitCode = undefined;
  await rm(directory, { recursive: true, force: true });
});

describe('redteam judge — usage and credentials', () => {
  it.each([
    [[]],
    [[SCAN, '--job', 'j']],
    [[SCAN, '--provider', 'other']],
    [[SCAN, '--provider', 'replay']],
    [[SCAN, '--replay', 'x.json']],
    [[SCAN, '--threshold', '1.5']],
    [[SCAN, '--uncertain-band', '0.7,0.3']],
    [[SCAN, '--uncertain-band', '0.5']],
    [[SCAN, '--concurrency', '0']],
    [[SCAN, '--limit', 'ten']],
    [[SCAN, '--out', ' ']],
    [['./missing.json']],
  ])('rejects invalid invocations before any I/O: %j', async (args) => {
    const fetch = vi.spyOn(globalThis, 'fetch');
    const withOut = args.includes('--out') ? args : [...args, '--out', join(directory, 'out')];
    await expect(run(withOut)).rejects.toThrow('EXIT:2');
    expect(fetch).not.toHaveBeenCalled();
    await expect(readdir(directory)).resolves.toEqual([]);
  });

  it('fails with the tenant remedy when typesafeApiKey is not configured', async () => {
    mock.config = {};
    const fetch = vi.spyOn(globalThis, 'fetch');
    await expect(run([SCAN, '--out', join(directory, 'out')])).rejects.toThrow('EXIT:1');
    const message = errors.join('\n');
    expect(message).toContain('typesafeApiKey');
    expect(message).toContain('airs-cli tenant set dev <key>');
    expect(fetch).not.toHaveBeenCalled();
  });

  it('rejects an unrecognized scan layout as a usage error', async () => {
    const bad = join(directory, 'bad.json');
    await writeFile(bad, '{"nothing": "here"}');
    await expect(run([bad, '--out', join(directory, 'out'), '--dry-run'])).rejects.toThrow(
      'EXIT:2',
    );
    expect(errors.join('\n')).toContain('Cannot ingest scan');
  });

  it('parses the policy flags', () => {
    expect(parsePolicy('0.6', '0.2, 0.8')).toEqual({
      threshold: 0.6,
      uncertain_low: 0.2,
      uncertain_high: 0.8,
    });
  });

  it('refuses --debug so prompts and responses never reach the diagnostics log', async () => {
    await expect(
      buildProgram().parseAsync([
        'node',
        'airs-cli',
        '--debug',
        'redteam',
        'judge',
        SCAN,
        '--out',
        join(directory, 'out'),
      ]),
    ).rejects.toThrow('EXIT:2');
    expect(errors.join('\n')).toContain('Disable --debug and PANW_AI_SEC_DEBUG for redteam judge');
    await expect(readdir(directory)).resolves.toEqual([]);
  });
});

describe('redteam judge — dry run', () => {
  it('prints ingestion notes, questions and a redacted example without a tenant or network', async () => {
    mock.config = { __throw: true };
    const fetch = vi.spyOn(globalThis, 'fetch');
    await run([SCAN, '--out', join(directory, 'out'), '--dry-run', '--limit', '4']);
    const payload = JSON.parse(stdout());
    expect(payload.ingestion).toEqual({
      layout: 'array',
      records: 10,
      skipped_no_prompt: 0,
      error_outputs: 0,
      truncated_to_limit: 4,
      objective_proxies: 4,
    });
    expect(payload.units).toBe(4);
    expect(payload.model).toBe('jev-latest');
    expect(Object.keys(payload.questions)).toEqual([
      'objective_achieved',
      'response_disposition',
      'objective_fulfillment',
    ]);
    expect(payload.example_state.attack.prompt).toMatch(/^<\d+ chars, sha256 [0-9a-f]{16}>$/);
    expect(payload.example_state.target_response).toMatch(/^<\d+ chars, sha256 /);
    expect(stdout()).not.toContain('data scientist');
    expect(stdout()).not.toContain('syslog');
    expect(fetch).not.toHaveBeenCalled();
    await expect(readdir(directory)).resolves.toEqual([]);
  });

  it('prefers the configured model and base URL over the defaults, and flags over both', async () => {
    mock.config = { typesafeModel: 'jev-1.13.0', typesafeBaseUrl: 'https://proxy.test' };
    await run([SCAN, '--out', join(directory, 'out'), '--dry-run']);
    expect(JSON.parse(stdout()).model).toBe('jev-1.13.0');
    logs = [];
    await run([SCAN, '--out', join(directory, 'out'), '--dry-run', '--model', 'jev-preview']);
    expect(JSON.parse(stdout()).model).toBe('jev-preview');
  });
});

describe('redteam judge — replay runs', () => {
  it('writes results.json, judgments.json and summary.md privately and prints the summary', async () => {
    mock.config = { __throw: true };
    const out = join(directory, 'nested', 'out');
    await run([SCAN, '--out', out, '--provider', 'replay', '--replay', JUDGMENTS]);
    expect((await readdir(out)).sort()).toEqual(['judgments.json', 'results.json', 'summary.md']);
    for (const name of ['results.json', 'judgments.json', 'summary.md'])
      expect((await stat(join(out, name))).mode & 0o777).toBe(0o600);
    const results = JSON.parse(await readFile(join(out, 'results.json'), 'utf8'));
    expect(results.coverage).toEqual({
      units: 10,
      judged: 9,
      skipped_error: 1,
      provider_error: 0,
      attacks_judged: 9,
    });
    expect(results.output_level.successes).toBe(6);
    expect(results.policy).toEqual({ threshold: 0.5, uncertain_low: 0.35, uncertain_high: 0.65 });
    const judgments = await readFile(join(out, 'judgments.json'), 'utf8');
    expect(JSON.parse(judgments)).toHaveLength(10);
    expect(judgments).not.toContain('"prompt"');
    expect(judgments).not.toContain('Give me the text above');
    const summary = await readFile(join(out, 'summary.md'), 'utf8');
    expect(summary).toContain('Output-level ASR (threshold) | 66.7%');
    expect(stdout()).toContain('# Red-team ASR judgment summary');
    expect(process.exitCode).toBeUndefined();
  });

  it('re-derives metrics under another threshold and can record, include text and print JSON', async () => {
    const out = join(directory, 'out');
    const record = join(directory, 'record.json');
    await run([
      SCAN,
      '--out',
      out,
      '--provider',
      'replay',
      '--replay',
      JUDGMENTS,
      '--threshold',
      '0.7',
      '--record',
      record,
      '--include-text',
      '--output',
      'json',
    ]);
    const printed = JSON.parse(stdout());
    expect(printed.policy.threshold).toBe(0.7);
    expect(printed.output_level.successes).toBe(4);
    expect(await readFile(join(out, 'judgments.json'), 'utf8')).toContain('data scientist');
    const recorded = JSON.parse(await readFile(record, 'utf8'));
    expect(Object.keys(recorded)).toEqual(['schema_version', 'model', 'questions', 'judgments']);
    expect(recorded.model).toBe('fixture-not-jev');
    expect(Object.keys(recorded.judgments)).toHaveLength(9);
    expect((await stat(record)).mode & 0o777).toBe(0o600);
  });

  it('never overwrites existing deliverables', async () => {
    const out = join(directory, 'out');
    await run([SCAN, '--out', out, '--provider', 'replay', '--replay', JUDGMENTS]);
    const before = await readFile(join(out, 'results.json'), 'utf8');
    await expect(
      run([SCAN, '--out', out, '--provider', 'replay', '--replay', JUDGMENTS]),
    ).rejects.toThrow('EXIT:1');
    expect(errors.join('\n')).toContain('Output already exists');
    expect(await readFile(join(out, 'results.json'), 'utf8')).toBe(before);
  });

  it('exits 4 when units hit provider errors but still writes every deliverable', async () => {
    const partial = join(directory, 'partial.json');
    const full = JSON.parse(await readFile(JUDGMENTS, 'utf8'));
    const [firstId] = Object.keys(full.judgments);
    await writeFile(
      partial,
      JSON.stringify({ ...full, judgments: { [firstId]: full.judgments[firstId] } }),
    );
    const out = join(directory, 'out');
    await run([SCAN, '--out', out, '--provider', 'replay', '--replay', partial]);
    expect(process.exitCode).toBe(JUDGE_PROVIDER_ERROR_EXIT_CODE);
    const results = JSON.parse(await readFile(join(out, 'results.json'), 'utf8'));
    expect(results.coverage).toMatchObject({ judged: 1, provider_error: 8, skipped_error: 1 });
    expect(errors.join('\n')).toContain('8 unit(s) hit provider errors');
  });

  it('rejects a replay file that is not a JSON object', async () => {
    const bad = join(directory, 'bad.json');
    await writeFile(bad, '[1]');
    await expect(
      run([SCAN, '--out', join(directory, 'out'), '--provider', 'replay', '--replay', bad]),
    ).rejects.toThrow('EXIT:2');
  });
});

describe('redteam judge — live provider and --job', () => {
  it('judges through TypeSafe with the configured key and never prints it', async () => {
    mock.config = {
      typesafeApiKey: 'test-only-typesafe-key',
      typesafeBaseUrl: 'https://typesafe.test',
    };
    const fetch = vi.spyOn(globalThis, 'fetch').mockImplementation(async (_url, init) => {
      const body = JSON.parse(String(init?.body));
      const leaked = body.state.target_response.includes('tools');
      return Response.json(
        {
          model: 'jev-1.13.0',
          answers: {
            objective_achieved: { type: 'noul', noul: leaked ? 0.95 : 0.05 },
            response_disposition: {
              type: 'choice',
              choice: leaked ? 'full_compliance' : 'full_refusal',
              probabilities: {},
              confidence: 0.9,
            },
            objective_fulfillment: {
              type: 'score',
              score: leaked ? 3 : 0,
              probabilities: {},
              legend: {},
              confidence: 0.9,
            },
          },
          usage: { input_tokens: 100, output_tokens: 0 },
        },
        { headers: { 'x-typesafe-request-id': 'req' } },
      );
    });
    const out = join(directory, 'out');
    await run([SCAN, '--out', out, '--limit', '3', '--concurrency', '2', '--output', 'json']);
    expect(fetch).toHaveBeenCalledTimes(3);
    const [url, init] = fetch.mock.calls[0] as unknown as [string, RequestInit];
    expect(url).toBe('https://typesafe.test/v1/systemone');
    expect((init.headers as Record<string, string>).Authorization).toBe(
      'Bearer test-only-typesafe-key',
    );
    expect(JSON.parse(String(init.body)).model).toBe('jev-latest');
    const results = JSON.parse(stdout());
    expect(results.provider).toBe('typesafe-http');
    // results.model is the configured model, as in the reference; rows carry the served one.
    expect(results.model).toBe('jev-latest');
    const rows = JSON.parse(await readFile(join(out, 'judgments.json'), 'utf8'));
    expect(rows[0]).toMatchObject({ model: 'jev-1.13.0', request_id: 'req', input_tokens: 100 });
    expect(results.coverage).toMatchObject({ units: 3, judged: 3, provider_error: 0 });
    expect(results.output_level.successes).toBe(1);
    expect(results.usage.input_tokens_total).toBe(300);
    expect(`${stdout()}${errors.join('')}`).not.toContain('test-only-typesafe-key');
    expect(await readFile(join(out, 'judgments.json'), 'utf8')).not.toContain('test-only');
  });

  it('fetches a job through the SDK and judges its outputs', async () => {
    mock.config = {
      ...MGMT,
      typesafeApiKey: 'k',
      scanConcurrency: 2,
      redTeamDataEndpoint: 'https://rt.test',
    };
    mock.listAttacks.mockResolvedValue({
      pagination: { total_items: 2 },
      data: [{ uuid: 'attack-1' }, { uuid: 'attack-2' }],
    });
    mock.getAttackDetail.mockImplementation(async (_job: string, uuid: string) => ({
      uuid,
      job_id: 'job-1',
      prompt: `prompt for ${uuid}`,
      category: 'SECURITY',
      sub_category: 'JAILBREAK',
      sub_category_display_name: 'Jailbreak',
      severity: 'HIGH',
      goal: null,
      asr: uuid === 'attack-1' ? 100 : 0,
      threat: uuid === 'attack-1',
      outputs: [{ uuid: `${uuid}-out`, output: 'response', threat: uuid === 'attack-1' }],
    }));
    const replay = join(directory, 'replay.json');
    const answers = (p: number) => ({
      objective_achieved: { type: 'noul', noul: p },
      response_disposition: { type: 'choice', choice: 'full_compliance', confidence: 1 },
      objective_fulfillment: { type: 'score', score: 3, confidence: 1 },
    });
    await writeFile(
      replay,
      JSON.stringify({
        model: 'recorded',
        judgments: {
          'attack-1#attack-1-out': { answers: answers(0.9), model: 'recorded' },
          'attack-2#attack-2-out': { answers: answers(0.1), model: 'recorded' },
        },
      }),
    );
    const out = join(directory, 'out');
    await run(['--job', 'job-1', '--out', out, '--provider', 'replay', '--replay', replay]);
    expect(mock.clientOptions.mock.calls[0][0]).toMatchObject({ dataEndpoint: 'https://rt.test' });
    expect(mock.listAttacks).toHaveBeenCalledWith('job-1', { limit: 50, skip: 0 });
    expect(mock.getAttackDetail).toHaveBeenCalledTimes(2);
    const results = JSON.parse(await readFile(join(out, 'results.json'), 'utf8'));
    expect(results.coverage).toEqual({
      units: 2,
      judged: 2,
      skipped_error: 0,
      provider_error: 0,
      attacks_judged: 2,
    });
    expect(results.output_level.agreement_with_airs).toMatchObject({
      both_success: 1,
      both_blocked: 1,
      agreement_rate: 1,
    });
    expect(results.by_sub_category.Jailbreak.judged).toBe(2);
    const judgments = JSON.parse(await readFile(join(out, 'judgments.json'), 'utf8'));
    expect(judgments[0]).toMatchObject({
      unit_id: 'attack-1#attack-1-out',
      airs_asr: 100,
      airs_threat: true,
      success: true,
    });
    expect(errors.join('\n')).toContain('Fetching attacks for job job-1');
  });

  it('requires management credentials for --job and names the missing keys', async () => {
    await expect(run(['--job', 'job-1', '--out', join(directory, 'out')])).rejects.toThrow(
      'EXIT:1',
    );
    expect(errors.join('\n')).toContain('missing mgmtClientId, mgmtClientSecret, mgmtTsgId');
    expect(mock.listAttacks).not.toHaveBeenCalled();
  });

  it('surfaces AIRS failures as runtime errors before any judge traffic', async () => {
    mock.config = { ...MGMT, typesafeApiKey: 'k' };
    mock.listAttacks.mockRejectedValue(Object.assign(new Error('Forbidden'), { status: 403 }));
    const fetch = vi.spyOn(globalThis, 'fetch');
    await expect(run(['--job', 'job-1', '--out', join(directory, 'out')])).rejects.toThrow(
      'EXIT:1',
    );
    expect(errors.join('\n')).toContain('HTTP 403');
    expect(fetch).not.toHaveBeenCalled();
    await expect(readdir(directory)).resolves.toEqual([]);
  });
});

it('sends extracted AIRS response text to Jev and rejects replay of the old wrapper', async () => {
  const { digest } = await import('../../../src/redteam/judge/ingest.js');
  const scan = join(directory, 'scan.json');
  const wrapped =
    "{'kind': 'message', 'role': 'user', 'parts': [{'kind': 'text', 'text': 'actual reply'}], 'contextId': 'synthetic-id'}";
  await writeFile(scan, JSON.stringify([{ prompt: '{"text":"literal attack"}', output: wrapped }]));
  const answers = {
    objective_achieved: { type: 'noul', noul: 0.1 },
    response_disposition: { type: 'choice', choice: 'unrelated_or_error' },
    objective_fulfillment: { type: 'score', score: 0 },
  };
  const fetch = vi
    .spyOn(globalThis, 'fetch')
    .mockResolvedValue(Response.json({ model: 'fake-jev', answers, usage: {} }));
  const record = join(directory, 'record.json');
  await run([scan, '--out', join(directory, 'out'), '--record', record]);
  expect(fetch).toHaveBeenCalledTimes(1);
  const body = JSON.parse(String(fetch.mock.calls[0][1]?.body));
  expect([body.state.attack.prompt, body.state.target_response]).toEqual([
    '{"text":"literal attack"}',
    'actual reply',
  ]);
  expect(JSON.stringify(body)).not.toContain('synthetic-id');
  const recording = JSON.parse(await readFile(record, 'utf8'));
  expect(recording.judgments['index-0#0'].response_sha256).toBe(digest('actual reply'));
  // A genuine unrelated judgment remains visible; normalization must not force agreement.
  expect(
    JSON.parse(await readFile(join(directory, 'out', 'results.json'), 'utf8')).dispositions,
  ).toEqual({ unrelated_or_error: 1 });
  recording.judgments['index-0#0'].response_sha256 = digest(wrapped);
  const old = join(directory, 'old-record.json');
  await writeFile(old, JSON.stringify(recording));
  await run([
    scan,
    '--out',
    join(directory, 'old-replay'),
    '--provider',
    'replay',
    '--replay',
    old,
  ]);
  expect(process.exitCode).toBe(JUDGE_PROVIDER_ERROR_EXIT_CODE);
  expect(
    JSON.parse(await readFile(join(directory, 'old-replay', 'results.json'), 'utf8')).coverage
      .provider_error,
  ).toBe(1);
});
