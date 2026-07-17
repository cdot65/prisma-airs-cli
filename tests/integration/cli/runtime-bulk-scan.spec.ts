import * as fs from 'node:fs/promises';
import * as os from 'node:os';
import * as path from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const sdk = vi.hoisted(() => ({
  asyncScan: vi.fn(),
  queryByReportIds: vi.fn(),
  queryByScanIds: vi.fn(),
  syncScan: vi.fn(),
}));

vi.mock('@cdot65/prisma-airs-sdk', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@cdot65/prisma-airs-sdk')>();
  return {
    ...actual,
    init: vi.fn(),
    Scanner: vi.fn(() => sdk),
  };
});

import { saveBulkScanState } from '../../../src/cli/bulk-scan-state.js';
import { buildProgram } from '../../../src/cli/program.js';
import { setQuiet } from '../../../src/cli/renderer/ui.js';

describe('runtime bulk-scan', () => {
  let tmpDir: string;

  beforeEach(async () => {
    vi.clearAllMocks();
    process.exitCode = undefined;
    vi.spyOn(console, 'error').mockImplementation(() => {});
    vi.spyOn(console, 'log').mockImplementation(() => {});
    tmpDir = await fs.mkdtemp(path.join(os.tmpdir(), 'airs-bulk-scan-'));
    vi.stubEnv('PANW_AI_SEC_API_KEY', 'test-api-key');
    vi.stubEnv('PRISMA_AIRS_CONFIG_PATH', path.join(tmpDir, 'missing-config.json'));
    vi.stubEnv('DATA_DIR', path.join(tmpDir, 'runs'));
  });

  afterEach(async () => {
    process.exitCode = undefined;
    setQuiet(false);
    vi.unstubAllEnvs();
    vi.restoreAllMocks();
    await fs.rm(tmpDir, { recursive: true, force: true });
  });

  it('writes one correctly aligned result for every input prompt', async () => {
    sdk.asyncScan.mockResolvedValueOnce({
      received: '2026-07-17T00:00:00Z',
      scan_id: 'scan-a',
    });
    const fixtureRows = [
      completeRow('scan-a', 3, 'allow', 'benign'),
      completeRow('scan-a', 4, 'allow', 'benign'),
      completeRow('scan-a', 0, 'block', 'malicious', { source_code: true }),
      completeRow('scan-a', 1, 'block', 'malicious', { agent: true }),
      completeRow('scan-a', 2, 'allow', 'benign'),
      completeRow('scan-a', 6, 'allow', 'benign'),
      completeRow('scan-a', 5, 'allow', 'benign'),
    ];
    sdk.queryByScanIds.mockImplementation(async (scanIds: string[]) =>
      fixtureRows.filter((row) => scanIds.includes(row.scan_id)),
    );

    const inputPath = path.join(tmpDir, 'prompts.txt');
    const outputPath = path.join(tmpDir, 'results.csv');
    await fs.writeFile(
      inputPath,
      Array.from({ length: 7 }, (_, index) => `distinct prompt ${index}`).join('\n'),
      'utf-8',
    );

    await buildProgram().parseAsync([
      'node',
      'airs',
      '--quiet',
      'runtime',
      'bulk-scan',
      '--profile',
      'bulk-profile',
      '--file',
      inputPath,
      '--output-file',
      outputPath,
    ]);

    const [header, ...csvRows] = (await fs.readFile(outputPath, 'utf-8')).trimEnd().split('\n');
    expect(header).toContain('source_code');
    expect(header).toContain('agent');
    expect(csvRows).toHaveLength(7);
    expect(csvRows.map((row) => row.split(',')[0])).toEqual(
      Array.from({ length: 7 }, (_, index) => `"distinct prompt ${index}"`),
    );
    expect(csvRows[0]).toContain('"block","malicious","true"');
    expect(csvRows[0]).toContain('"true"');
    expect(csvRows[1]).toContain('"block","malicious","true"');
    expect(csvRows.slice(2).every((row) => row.includes('"allow","benign","false"'))).toBe(true);
  });

  it('resume-poll restores prompts and completes a scan after polling fails', async () => {
    sdk.asyncScan.mockResolvedValueOnce({
      received: '2026-07-17T00:00:00Z',
      scan_id: 'scan-resume',
    });
    sdk.queryByScanIds.mockRejectedValueOnce(new Error('temporary polling failure'));

    const inputPath = path.join(tmpDir, 'resume-prompts.txt');
    const outputPath = path.join(tmpDir, 'resumed.csv');
    await fs.writeFile(inputPath, 'resume prompt 0\nresume prompt 1', 'utf-8');

    const exit = vi.spyOn(process, 'exit').mockImplementation(((code?: number) => {
      throw new ProcessExitError(code ?? 0);
    }) as typeof process.exit);

    await expect(
      buildProgram().parseAsync([
        'node',
        'airs',
        '--quiet',
        'runtime',
        'bulk-scan',
        '--profile',
        'bulk-profile',
        '--file',
        inputPath,
        '--output-file',
        outputPath,
      ]),
    ).rejects.toMatchObject({ exitCode: 1 });

    const stateDir = path.join(tmpDir, 'bulk-scans');
    const [stateName] = await fs.readdir(stateDir);
    const statePath = path.join(stateDir, stateName);

    sdk.queryByScanIds.mockReset();
    sdk.queryByScanIds.mockResolvedValueOnce([
      completeRow('scan-resume', 1, 'allow', 'benign'),
      completeRow('scan-resume', 0, 'block', 'malicious', { source_code: true }),
    ]);

    await buildProgram().parseAsync([
      'node',
      'airs',
      '--quiet',
      'runtime',
      'resume-poll',
      statePath,
      '--output-file',
      outputPath,
    ]);

    expect(exit).toHaveBeenCalledOnce();
    const [, ...csvRows] = (await fs.readFile(outputPath, 'utf-8')).trimEnd().split('\n');
    expect(csvRows).toHaveLength(2);
    expect(csvRows[0]).toContain('"resume prompt 0","block","malicious","true"');
    expect(csvRows[1]).toContain('"resume prompt 1","allow","benign","false"');
  });

  it('prevents overlapping resume processes from submitting the same pending prompt', async () => {
    const outputPath = path.join(tmpDir, 'concurrent-resume.csv');
    const createdAt = new Date().toISOString();
    const statePath = await saveBulkScanState(
      {
        version: 2,
        profile: 'bulk-profile',
        outputFile: outputPath,
        batchSize: 25,
        createdAt,
        updatedAt: createdAt,
        items: [{ index: 0, reqId: 0, prompt: 'submit once', status: 'pending' }],
      },
      path.join(tmpDir, 'bulk-scans'),
    );
    let resolveSubmission: ((receipt: { received: string; scan_id: string }) => void) | undefined;
    sdk.asyncScan.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveSubmission = resolve;
        }),
    );
    sdk.queryByScanIds.mockResolvedValueOnce([
      completeRow('scan-concurrent', 0, 'allow', 'benign'),
    ]);
    vi.spyOn(process, 'exit').mockImplementation(((code?: number) => {
      throw new ProcessExitError(code ?? 0);
    }) as typeof process.exit);
    const args = [
      'node',
      'airs',
      '--quiet',
      'runtime',
      'resume-poll',
      statePath,
      '--output-file',
      outputPath,
    ];

    const firstResume = buildProgram().parseAsync(args);
    await vi.waitFor(() => expect(sdk.asyncScan).toHaveBeenCalledOnce());
    await expect(fs.access(`${statePath}.lock`)).resolves.toBeUndefined();

    await expect(buildProgram().parseAsync(args)).rejects.toMatchObject({ exitCode: 1 });
    expect(sdk.asyncScan).toHaveBeenCalledOnce();

    resolveSubmission?.({ received: '2026-07-17T00:00:00Z', scan_id: 'scan-concurrent' });
    await firstResume;
    await expect(fs.access(`${statePath}.lock`)).rejects.toMatchObject({ code: 'ENOENT' });
    const [, ...csvRows] = (await fs.readFile(outputPath, 'utf-8')).trimEnd().split('\n');
    expect(csvRows).toHaveLength(1);
  });

  it('resume-poll submits only unfinished prompts after an exhausted 429', async () => {
    const rateLimit = Object.assign(new Error('rate limited'), {
      failureKind: 'http',
      statusCode: 429,
      retryAfterMs: 0,
    });
    sdk.asyncScan
      .mockResolvedValueOnce({ received: '2026-07-17T00:00:00Z', scan_id: 'scan-first' })
      .mockRejectedValue(rateLimit);

    const inputPath = path.join(tmpDir, 'rate-limited-prompts.txt');
    const outputPath = path.join(tmpDir, 'rate-limited.csv');
    await fs.writeFile(
      inputPath,
      Array.from({ length: 21 }, (_, index) => `rate prompt ${index}`).join('\n'),
      'utf-8',
    );
    const exit = vi.spyOn(process, 'exit').mockImplementation(((code?: number) => {
      throw new ProcessExitError(code ?? 0);
    }) as typeof process.exit);

    await expect(
      buildProgram().parseAsync([
        'node',
        'airs',
        '--quiet',
        'runtime',
        'bulk-scan',
        '--profile',
        'bulk-profile',
        '--file',
        inputPath,
        '--output-file',
        outputPath,
      ]),
    ).rejects.toMatchObject({ exitCode: 1 });

    const stateDir = path.join(tmpDir, 'bulk-scans');
    const [stateName] = await fs.readdir(stateDir);
    const statePath = path.join(stateDir, stateName);

    sdk.asyncScan.mockReset();
    sdk.asyncScan.mockResolvedValueOnce({
      received: '2026-07-17T00:00:01Z',
      scan_id: 'scan-second',
    });
    const rows = Array.from({ length: 21 }, (_, reqId) =>
      completeRow(reqId < 20 ? 'scan-first' : 'scan-second', reqId, 'allow', 'benign'),
    );
    sdk.queryByScanIds.mockImplementation(async (scanIds: string[]) =>
      rows.filter((row) => scanIds.includes(row.scan_id)),
    );

    await buildProgram().parseAsync([
      'node',
      'airs',
      '--quiet',
      'runtime',
      'resume-poll',
      statePath,
      '--output-file',
      outputPath,
    ]);

    expect(exit).toHaveBeenCalledOnce();
    expect(sdk.asyncScan).toHaveBeenCalledOnce();
    expect(sdk.asyncScan.mock.calls[0][0].map((item: { req_id: number }) => item.req_id)).toEqual([
      20,
    ]);
    const [, ...csvRows] = (await fs.readFile(outputPath, 'utf-8')).trimEnd().split('\n');
    expect(csvRows).toHaveLength(21);
  });

  it('allows a safely rejected 4xx submission to be retried on resume', async () => {
    sdk.asyncScan.mockRejectedValueOnce(
      Object.assign(new Error('invalid profile'), {
        failureKind: 'http',
        statusCode: 400,
      }),
    );

    const inputPath = path.join(tmpDir, 'rejected-prompts.txt');
    const outputPath = path.join(tmpDir, 'rejected.csv');
    await fs.writeFile(inputPath, 'retry after fixing the request', 'utf-8');
    vi.spyOn(process, 'exit').mockImplementation(((code?: number) => {
      throw new ProcessExitError(code ?? 0);
    }) as typeof process.exit);

    await expect(
      buildProgram().parseAsync([
        'node',
        'airs',
        '--quiet',
        'runtime',
        'bulk-scan',
        '--profile',
        'bulk-profile',
        '--file',
        inputPath,
        '--output-file',
        outputPath,
      ]),
    ).rejects.toMatchObject({ exitCode: 1 });

    const [stateName] = await fs.readdir(path.join(tmpDir, 'bulk-scans'));
    const statePath = path.join(tmpDir, 'bulk-scans', stateName);
    expect(JSON.parse(await fs.readFile(statePath, 'utf-8')).items[0].status).toBe('pending');

    sdk.asyncScan.mockReset();
    sdk.asyncScan.mockResolvedValueOnce({
      received: '2026-07-17T00:00:01Z',
      scan_id: 'scan-after-fix',
    });
    sdk.queryByScanIds.mockResolvedValueOnce([completeRow('scan-after-fix', 0, 'allow', 'benign')]);

    await buildProgram().parseAsync([
      'node',
      'airs',
      '--quiet',
      'runtime',
      'resume-poll',
      statePath,
      '--output-file',
      outputPath,
    ]);

    expect(sdk.asyncScan).toHaveBeenCalledOnce();
    const [, ...rows] = (await fs.readFile(outputPath, 'utf-8')).trimEnd().split('\n');
    expect(rows).toHaveLength(1);
  });

  it('writes partial results but exits nonzero when a prompt fails', async () => {
    sdk.asyncScan.mockResolvedValueOnce({
      received: '2026-07-17T00:00:00Z',
      scan_id: 'scan-partial',
    });
    sdk.queryByScanIds.mockResolvedValueOnce([
      completeRow('scan-partial', 0, 'allow', 'benign'),
      { scan_id: 'scan-partial', req_id: 1, status: 'failed' },
    ]);

    const inputPath = path.join(tmpDir, 'partial-prompts.txt');
    const outputPath = path.join(tmpDir, 'partial.csv');
    await fs.writeFile(inputPath, 'successful prompt\nfailed prompt', 'utf-8');

    await buildProgram().parseAsync([
      'node',
      'airs',
      '--quiet',
      'runtime',
      'bulk-scan',
      '--profile',
      'bulk-profile',
      '--file',
      inputPath,
      '--output-file',
      outputPath,
    ]);

    expect(process.exitCode).toBe(1);
    const [, ...csvRows] = (await fs.readFile(outputPath, 'utf-8')).trimEnd().split('\n');
    expect(csvRows).toHaveLength(2);
    expect(csvRows[0]).toContain('"allow","benign"');
    expect(csvRows[1]).toContain('"failed","error"');
    expect(csvRows[1]).not.toContain('"allow"');
  });

  it('preserves partial results and exits nonzero when a resumed prompt fails', async () => {
    sdk.asyncScan.mockResolvedValueOnce({
      received: '2026-07-17T00:00:00Z',
      scan_id: 'scan-resumed-partial',
    });
    sdk.queryByScanIds.mockRejectedValueOnce(new Error('temporary polling failure'));

    const inputPath = path.join(tmpDir, 'resumed-partial-prompts.txt');
    const outputPath = path.join(tmpDir, 'resumed-partial.csv');
    await fs.writeFile(inputPath, 'successful prompt\nfailed prompt', 'utf-8');
    vi.spyOn(process, 'exit').mockImplementation(((code?: number) => {
      throw new ProcessExitError(code ?? 0);
    }) as typeof process.exit);

    await expect(
      buildProgram().parseAsync([
        'node',
        'airs',
        '--quiet',
        'runtime',
        'bulk-scan',
        '--profile',
        'bulk-profile',
        '--file',
        inputPath,
        '--output-file',
        outputPath,
      ]),
    ).rejects.toMatchObject({ exitCode: 1 });

    const [stateName] = await fs.readdir(path.join(tmpDir, 'bulk-scans'));
    const statePath = path.join(tmpDir, 'bulk-scans', stateName);
    sdk.queryByScanIds.mockReset();
    sdk.queryByScanIds.mockResolvedValueOnce([
      completeRow('scan-resumed-partial', 0, 'allow', 'benign'),
      { scan_id: 'scan-resumed-partial', req_id: 1, status: 'failed' },
    ]);
    process.exitCode = undefined;

    await buildProgram().parseAsync([
      'node',
      'airs',
      '--quiet',
      'runtime',
      'resume-poll',
      statePath,
      '--output-file',
      outputPath,
    ]);

    expect(process.exitCode).toBe(1);
    const [, ...csvRows] = (await fs.readFile(outputPath, 'utf-8')).trimEnd().split('\n');
    expect(csvRows).toHaveLength(2);
    expect(csvRows[0]).toContain('"allow","benign"');
    expect(csvRows[1]).toContain('"failed","error"');
  });

  it('records an ambiguous POST outcome and refuses to resubmit it', async () => {
    const ambiguousFailure = Object.assign(new Error('socket closed after upstream 429'), {
      failureKind: 'network',
    });
    sdk.asyncScan.mockRejectedValueOnce(ambiguousFailure);

    const inputPath = path.join(tmpDir, 'ambiguous-prompts.txt');
    const outputPath = path.join(tmpDir, 'ambiguous.csv');
    await fs.writeFile(inputPath, 'ambiguous prompt 0\nambiguous prompt 1', 'utf-8');
    const exit = vi.spyOn(process, 'exit').mockImplementation(((code?: number) => {
      throw new ProcessExitError(code ?? 0);
    }) as typeof process.exit);

    await expect(
      buildProgram().parseAsync([
        'node',
        'airs',
        '--quiet',
        'runtime',
        'bulk-scan',
        '--profile',
        'bulk-profile',
        '--file',
        inputPath,
        '--output-file',
        outputPath,
      ]),
    ).rejects.toMatchObject({ exitCode: 1 });

    const [stateName] = await fs.readdir(path.join(tmpDir, 'bulk-scans'));
    const statePath = path.join(tmpDir, 'bulk-scans', stateName);
    const state = JSON.parse(await fs.readFile(statePath, 'utf-8'));
    expect(state.items.map((item: { status: string }) => item.status)).toEqual([
      'ambiguous',
      'ambiguous',
    ]);
    expect(sdk.asyncScan).toHaveBeenCalledOnce();

    sdk.asyncScan.mockClear();
    await expect(
      buildProgram().parseAsync([
        'node',
        'airs',
        '--quiet',
        'runtime',
        'resume-poll',
        statePath,
        '--output-file',
        outputPath,
      ]),
    ).rejects.toMatchObject({ exitCode: 1 });
    expect(sdk.asyncScan).not.toHaveBeenCalled();
    expect(sdk.queryByScanIds).not.toHaveBeenCalled();
    expect(exit).toHaveBeenCalledTimes(2);
  });

  it('replaces stale output with the current empty projection before the first POST', async () => {
    sdk.asyncScan.mockRejectedValueOnce(
      Object.assign(new Error('connection reset'), { failureKind: 'network' }),
    );

    const inputPath = path.join(tmpDir, 'stale-output-prompts.txt');
    const outputPath = path.join(tmpDir, 'stale-output.csv');
    await fs.writeFile(inputPath, 'new job prompt', 'utf-8');
    await fs.writeFile(outputPath, 'old,misleading,data\n', 'utf-8');
    vi.spyOn(process, 'exit').mockImplementation(((code?: number) => {
      throw new ProcessExitError(code ?? 0);
    }) as typeof process.exit);

    await expect(
      buildProgram().parseAsync([
        'node',
        'airs',
        '--quiet',
        'runtime',
        'bulk-scan',
        '--profile',
        'bulk-profile',
        '--file',
        inputPath,
        '--output-file',
        outputPath,
      ]),
    ).rejects.toMatchObject({ exitCode: 1 });

    const output = await fs.readFile(outputPath, 'utf-8');
    expect(output).toMatch(/^prompt,action,category,triggered,/);
    expect(output).not.toContain('old,misleading,data');
    expect(output.split('\n')).toHaveLength(1);
  });

  it('recovers known accepted work before reporting a later ambiguous submission', async () => {
    const ambiguousFailure = Object.assign(new Error('connection reset after request write'), {
      failureKind: 'network',
    });
    sdk.asyncScan
      .mockResolvedValueOnce({ received: '2026-07-17T00:00:00Z', scan_id: 'scan-known' })
      .mockRejectedValueOnce(ambiguousFailure);

    const inputPath = path.join(tmpDir, 'partly-ambiguous-prompts.txt');
    const outputPath = path.join(tmpDir, 'partly-ambiguous.csv');
    await fs.writeFile(
      inputPath,
      Array.from({ length: 21 }, (_, index) => `partly ambiguous prompt ${index}`).join('\n'),
      'utf-8',
    );
    vi.spyOn(process, 'exit').mockImplementation(((code?: number) => {
      throw new ProcessExitError(code ?? 0);
    }) as typeof process.exit);

    await expect(
      buildProgram().parseAsync([
        'node',
        'airs',
        '--quiet',
        'runtime',
        'bulk-scan',
        '--profile',
        'bulk-profile',
        '--file',
        inputPath,
        '--output-file',
        outputPath,
      ]),
    ).rejects.toMatchObject({ exitCode: 1 });

    const [stateName] = await fs.readdir(path.join(tmpDir, 'bulk-scans'));
    const statePath = path.join(tmpDir, 'bulk-scans', stateName);
    sdk.asyncScan.mockClear();
    sdk.queryByScanIds.mockResolvedValueOnce(
      Array.from({ length: 20 }, (_, reqId) => completeRow('scan-known', reqId, 'allow', 'benign')),
    );

    await expect(
      buildProgram().parseAsync([
        'node',
        'airs',
        '--quiet',
        'runtime',
        'resume-poll',
        statePath,
        '--output-file',
        outputPath,
      ]),
    ).rejects.toMatchObject({ exitCode: 1 });

    expect(sdk.asyncScan).not.toHaveBeenCalled();
    expect(sdk.queryByScanIds).toHaveBeenCalledOnce();
    const [, ...csvRows] = (await fs.readFile(outputPath, 'utf-8')).trimEnd().split('\n');
    expect(csvRows).toHaveLength(20);
  });

  it('polls accepted receipts before retrying pending submissions on resume', async () => {
    const definiteRejection = Object.assign(new Error('bad request'), {
      failureKind: 'http',
      statusCode: 400,
    });
    sdk.asyncScan
      .mockResolvedValueOnce({ received: '2026-07-17T00:00:00Z', scan_id: 'scan-accepted' })
      .mockRejectedValueOnce(definiteRejection);

    const inputPath = path.join(tmpDir, 'accepted-plus-pending-prompts.txt');
    const outputPath = path.join(tmpDir, 'accepted-plus-pending.csv');
    await fs.writeFile(
      inputPath,
      Array.from({ length: 21 }, (_, index) => `accepted plus pending ${index}`).join('\n'),
      'utf-8',
    );
    vi.spyOn(process, 'exit').mockImplementation(((code?: number) => {
      throw new ProcessExitError(code ?? 0);
    }) as typeof process.exit);

    await expect(
      buildProgram().parseAsync([
        'node',
        'airs',
        '--quiet',
        'runtime',
        'bulk-scan',
        '--profile',
        'bulk-profile',
        '--file',
        inputPath,
        '--output-file',
        outputPath,
      ]),
    ).rejects.toMatchObject({ exitCode: 1 });

    const [stateName] = await fs.readdir(path.join(tmpDir, 'bulk-scans'));
    const statePath = path.join(tmpDir, 'bulk-scans', stateName);
    sdk.asyncScan.mockReset();
    sdk.asyncScan.mockRejectedValueOnce(
      Object.assign(new Error('connection reset after request write'), { failureKind: 'network' }),
    );
    sdk.queryByScanIds.mockResolvedValueOnce(
      Array.from({ length: 20 }, (_, reqId) =>
        completeRow('scan-accepted', reqId, 'allow', 'benign'),
      ),
    );

    await expect(
      buildProgram().parseAsync([
        'node',
        'airs',
        '--quiet',
        'runtime',
        'resume-poll',
        statePath,
        '--output-file',
        outputPath,
      ]),
    ).rejects.toMatchObject({ exitCode: 1 });

    expect(sdk.queryByScanIds).toHaveBeenCalledOnce();
    expect(sdk.asyncScan).toHaveBeenCalledOnce();
    expect(sdk.queryByScanIds.mock.invocationCallOrder[0]).toBeLessThan(
      sdk.asyncScan.mock.invocationCallOrder[0],
    );
    const [, ...csvRows] = (await fs.readFile(outputPath, 'utf-8')).trimEnd().split('\n');
    expect(csvRows).toHaveLength(20);
    const state = JSON.parse(await fs.readFile(statePath, 'utf-8'));
    expect(state.items.map((item: { status: string }) => item.status)).toEqual([
      ...Array.from({ length: 20 }, () => 'complete'),
      'ambiguous',
    ]);
  });

  it('rebuilds output from state without duplicates after a later batch fails', async () => {
    sdk.asyncScan
      .mockResolvedValueOnce({ received: '2026-07-17T00:00:00Z', scan_id: 'scan-complete' })
      .mockResolvedValueOnce({ received: '2026-07-17T00:00:01Z', scan_id: 'scan-pending' });
    const completeRows = Array.from({ length: 20 }, (_, reqId) =>
      completeRow('scan-complete', reqId, 'allow', 'benign'),
    );
    sdk.queryByScanIds.mockImplementation(async (scanIds: string[]) => {
      if (scanIds.includes('scan-complete')) return completeRows;
      throw new Error('later batch polling failed');
    });

    const inputPath = path.join(tmpDir, 'checkpoint-prompts.txt');
    const outputPath = path.join(tmpDir, 'checkpoint.csv');
    await fs.writeFile(
      inputPath,
      Array.from({ length: 21 }, (_, index) => `checkpoint prompt ${index}`).join('\n'),
      'utf-8',
    );
    const exit = vi.spyOn(process, 'exit').mockImplementation(((code?: number) => {
      throw new ProcessExitError(code ?? 0);
    }) as typeof process.exit);

    await expect(
      buildProgram().parseAsync([
        'node',
        'airs',
        '--quiet',
        'runtime',
        'bulk-scan',
        '--profile',
        'bulk-profile',
        '--file',
        inputPath,
        '--output-file',
        outputPath,
      ]),
    ).rejects.toMatchObject({ exitCode: 1 });

    const [, ...partialRows] = (await fs.readFile(outputPath, 'utf-8')).trimEnd().split('\n');
    expect(partialRows).toHaveLength(20);

    const [stateName] = await fs.readdir(path.join(tmpDir, 'bulk-scans'));
    const statePath = path.join(tmpDir, 'bulk-scans', stateName);
    sdk.queryByScanIds.mockReset();
    sdk.queryByScanIds.mockResolvedValueOnce([completeRow('scan-pending', 20, 'allow', 'benign')]);

    await buildProgram().parseAsync([
      'node',
      'airs',
      '--quiet',
      'runtime',
      'resume-poll',
      statePath,
      '--output-file',
      outputPath,
    ]);
    const firstResume = await fs.readFile(outputPath, 'utf-8');
    expect(firstResume.trimEnd().split('\n')).toHaveLength(22);

    sdk.queryByScanIds.mockClear();
    await buildProgram().parseAsync([
      'node',
      'airs',
      '--quiet',
      'runtime',
      'resume-poll',
      statePath,
      '--output-file',
      outputPath,
    ]);
    expect(await fs.readFile(outputPath, 'utf-8')).toBe(firstResume);
    expect(sdk.queryByScanIds).not.toHaveBeenCalled();
    expect(exit).toHaveBeenCalledOnce();
  });

  it('checkpoints prompt results incrementally within one SDK receipt', async () => {
    sdk.asyncScan.mockResolvedValueOnce({
      received: '2026-07-17T00:00:00Z',
      scan_id: 'scan-incremental',
    });
    sdk.queryByScanIds.mockResolvedValueOnce({
      *[Symbol.iterator]() {
        yield completeRow('scan-incremental', 0, 'allow', 'benign');
        throw new Error('polling response iteration failed');
      },
    });

    const inputPath = path.join(tmpDir, 'incremental-prompts.txt');
    const outputPath = path.join(tmpDir, 'incremental.csv');
    await fs.writeFile(inputPath, 'completed first\nstill pending', 'utf-8');
    vi.spyOn(process, 'exit').mockImplementation(((code?: number) => {
      throw new ProcessExitError(code ?? 0);
    }) as typeof process.exit);

    await expect(
      buildProgram().parseAsync([
        'node',
        'airs',
        '--quiet',
        'runtime',
        'bulk-scan',
        '--profile',
        'bulk-profile',
        '--file',
        inputPath,
        '--output-file',
        outputPath,
      ]),
    ).rejects.toMatchObject({ exitCode: 1 });

    const [, ...partialRows] = (await fs.readFile(outputPath, 'utf-8')).trimEnd().split('\n');
    expect(partialRows).toHaveLength(1);
    expect(partialRows[0]).toContain('"completed first"');
    const [stateName] = await fs.readdir(path.join(tmpDir, 'bulk-scans'));
    const statePath = path.join(tmpDir, 'bulk-scans', stateName);
    const state = JSON.parse(await fs.readFile(statePath, 'utf-8'));
    expect(state.items.map((item: { status: string }) => item.status)).toEqual([
      'complete',
      'submitted',
    ]);

    sdk.asyncScan.mockClear();
    sdk.queryByScanIds.mockReset();
    sdk.queryByScanIds.mockResolvedValueOnce([
      completeRow('scan-incremental', 1, 'block', 'malicious', { injection: true }),
      completeRow('scan-incremental', 0, 'allow', 'benign'),
    ]);

    await buildProgram().parseAsync([
      'node',
      'airs',
      '--quiet',
      'runtime',
      'resume-poll',
      statePath,
      '--output-file',
      outputPath,
    ]);

    expect(sdk.asyncScan).not.toHaveBeenCalled();
    const [, ...completeRows] = (await fs.readFile(outputPath, 'utf-8')).trimEnd().split('\n');
    expect(completeRows).toHaveLength(2);
    expect(completeRows[1]).toContain('"still pending","block","malicious","true"');
  });

  it.each([
    '0',
    '-1',
    '1.5',
    'abc',
    '9007199254740992',
  ])('rejects invalid batch size %s before submitting anything', async (batchSize) => {
    const inputPath = path.join(tmpDir, 'invalid-batch-size.txt');
    await fs.writeFile(inputPath, 'prompt', 'utf-8');
    vi.spyOn(process, 'exit').mockImplementation(((code?: number) => {
      throw new ProcessExitError(code ?? 0);
    }) as typeof process.exit);

    await expect(
      buildProgram().parseAsync([
        'node',
        'airs',
        '--quiet',
        'runtime',
        'bulk-scan',
        '--profile',
        'bulk-profile',
        '--file',
        inputPath,
        '--batch-size',
        batchSize,
      ]),
    ).rejects.toMatchObject({ exitCode: 2 });
    expect(sdk.asyncScan).not.toHaveBeenCalled();
  });

  it('finishes polling each sequential batch before submitting the next one', async () => {
    const events: string[] = [];
    sdk.asyncScan.mockImplementation(async (items: Array<{ req_id: number }>) => {
      const scanId = items[0].req_id === 0 ? 'scan-sequential-a' : 'scan-sequential-b';
      events.push(`submit:${items[0].req_id}`);
      return { received: '2026-07-17T00:00:00Z', scan_id: scanId };
    });
    const rows = Array.from({ length: 7 }, (_, reqId) =>
      completeRow(reqId < 5 ? 'scan-sequential-a' : 'scan-sequential-b', reqId, 'allow', 'benign'),
    );
    sdk.queryByScanIds.mockImplementation(async (scanIds: string[]) => {
      events.push(`poll:${scanIds[0]}`);
      return rows.filter((row) => scanIds.includes(row.scan_id));
    });

    const inputPath = path.join(tmpDir, 'sequential-prompts.txt');
    const outputPath = path.join(tmpDir, 'sequential.csv');
    await fs.writeFile(
      inputPath,
      Array.from({ length: 7 }, (_, index) => `sequential prompt ${index}`).join('\n'),
      'utf-8',
    );

    await buildProgram().parseAsync([
      'node',
      'airs',
      '--quiet',
      'runtime',
      'bulk-scan',
      '--profile',
      'bulk-profile',
      '--file',
      inputPath,
      '--output-file',
      outputPath,
      '--batch-size',
      '5',
    ]);

    expect(events).toEqual([
      'submit:0',
      'poll:scan-sequential-a',
      'submit:5',
      'poll:scan-sequential-b',
    ]);
  });

  it('preserves sequential batch sizing when a scan is resumed', async () => {
    sdk.asyncScan.mockResolvedValueOnce({
      received: '2026-07-17T00:00:00Z',
      scan_id: 'scan-resume-batch-0',
    });
    sdk.queryByScanIds.mockRejectedValueOnce(new Error('interrupt the first batch poll'));

    const inputPath = path.join(tmpDir, 'resume-batch-size-prompts.txt');
    const outputPath = path.join(tmpDir, 'resume-batch-size.csv');
    await fs.writeFile(
      inputPath,
      Array.from({ length: 6 }, (_, index) => `resume batch prompt ${index}`).join('\n'),
      'utf-8',
    );
    vi.spyOn(process, 'exit').mockImplementation(((code?: number) => {
      throw new ProcessExitError(code ?? 0);
    }) as typeof process.exit);

    await expect(
      buildProgram().parseAsync([
        'node',
        'airs',
        '--quiet',
        'runtime',
        'bulk-scan',
        '--profile',
        'bulk-profile',
        '--file',
        inputPath,
        '--output-file',
        outputPath,
        '--batch-size',
        '2',
      ]),
    ).rejects.toMatchObject({ exitCode: 1 });

    const [stateName] = await fs.readdir(path.join(tmpDir, 'bulk-scans'));
    const statePath = path.join(tmpDir, 'bulk-scans', stateName);
    const state = JSON.parse(await fs.readFile(statePath, 'utf-8'));
    expect(state.batchSize).toBe(2);

    const events: string[] = [];
    sdk.asyncScan.mockReset();
    sdk.asyncScan.mockImplementation(async (items: Array<{ req_id: number }>) => {
      events.push(`submit:${items.map((item) => item.req_id).join(',')}`);
      return {
        received: '2026-07-17T00:00:01Z',
        scan_id: `scan-resume-batch-${items[0].req_id}`,
      };
    });
    sdk.queryByScanIds.mockReset();
    sdk.queryByScanIds.mockImplementation(async (scanIds: string[]) => {
      const start = Number(scanIds[0].split('-').at(-1));
      events.push(`poll:${start}`);
      return [0, 1].map((offset) => completeRow(scanIds[0], start + offset, 'allow', 'benign'));
    });

    await buildProgram().parseAsync([
      'node',
      'airs',
      '--quiet',
      'runtime',
      'resume-poll',
      statePath,
      '--output-file',
      outputPath,
    ]);

    expect(events).toEqual(['poll:0', 'submit:2,3', 'poll:2', 'submit:4,5', 'poll:4']);
  });
});

class ProcessExitError extends Error {
  constructor(public readonly exitCode: number) {
    super(`process exited with ${exitCode}`);
  }
}

function completeRow(
  scanId: string,
  reqId: number,
  action: string,
  category: string,
  promptDetected: Record<string, boolean> = {},
) {
  return {
    scan_id: scanId,
    req_id: reqId,
    status: 'complete',
    result: {
      scan_id: scanId,
      report_id: `report-${scanId}`,
      action,
      category,
      prompt_detected: promptDetected,
    },
  };
}
