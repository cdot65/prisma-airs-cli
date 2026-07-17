import * as fs from 'node:fs/promises';
import * as path from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import {
  type BulkScanState,
  loadBulkScanState,
  saveBulkScanState,
} from '../../../src/cli/bulk-scan-state.js';

describe('bulk-scan-state', () => {
  let tmpDir: string;
  let state: BulkScanState;

  beforeEach(async () => {
    tmpDir = await fs.mkdtemp(path.join(import.meta.dirname ?? '/tmp', 'bss-'));
    state = {
      version: 2,
      profile: 'my-profile',
      outputFile: path.join(tmpDir, 'results.csv'),
      batchSize: 25,
      createdAt: '2026-07-17T00:00:00.000Z',
      updatedAt: '2026-07-17T00:00:00.000Z',
      items: [
        {
          index: 0,
          reqId: 0,
          prompt: 'first prompt',
          status: 'submitted',
          scanId: 'id-1',
        },
      ],
    };
  });

  afterEach(async () => {
    await fs.rm(tmpDir, { recursive: true, force: true });
  });

  describe('saveBulkScanState', () => {
    it('writes prompt correlation and profile to a JSON file', async () => {
      const filePath = await saveBulkScanState(state, tmpDir);

      const content = JSON.parse(await fs.readFile(filePath, 'utf-8'));
      expect(content.version).toBe(2);
      expect(content.profile).toBe('my-profile');
      expect(content.items[0]).toMatchObject({
        index: 0,
        reqId: 0,
        prompt: 'first prompt',
        scanId: 'id-1',
      });
      expect(content.updatedAt).toBeDefined();
    });

    it('creates file with .bulk-scan.json suffix', async () => {
      const filePath = await saveBulkScanState(state, tmpDir);

      expect(filePath).toMatch(/\.bulk-scan\.json$/);
    });

    it('creates the directory if it does not exist', async () => {
      const nestedDir = path.join(tmpDir, 'nested', 'deep');
      const filePath = await saveBulkScanState(state, nestedDir);

      const content = JSON.parse(await fs.readFile(filePath, 'utf-8'));
      expect(content.items[0].scanId).toBe('id-1');
    });

    it('atomically rewrites the same state path instead of creating another job', async () => {
      const filePath = await saveBulkScanState(state, tmpDir);
      state.profile = 'updated-profile';

      const rewrittenPath = await saveBulkScanState(state, tmpDir, filePath);

      expect(rewrittenPath).toBe(filePath);
      expect((await loadBulkScanState(filePath)).profile).toBe('updated-profile');
      expect((await fs.readdir(tmpDir)).filter((name) => name.endsWith('.json'))).toEqual([
        path.basename(filePath),
      ]);
    });

    it('refuses to persist state that cannot be safely resumed', async () => {
      state.batchSize = 0;

      await expect(saveBulkScanState(state, tmpDir)).rejects.toThrow(/invalid bulk-scan state/i);
      expect(await fs.readdir(tmpDir)).toEqual([]);
    });

    it.skipIf(process.platform === 'win32')(
      'protects persisted prompts with owner-only access',
      async () => {
        const nestedDir = path.join(tmpDir, 'private-state');
        const filePath = await saveBulkScanState(state, nestedDir);

        expect((await fs.stat(nestedDir)).mode & 0o777).toBe(0o700);
        expect((await fs.stat(filePath)).mode & 0o777).toBe(0o600);
      },
    );
  });

  describe('loadBulkScanState', () => {
    it('reads saved state from a JSON file', async () => {
      state.profile = 'test-profile';
      const filePath = await saveBulkScanState(state, tmpDir);

      const loaded = await loadBulkScanState(filePath);
      expect(loaded.items[0].scanId).toBe('id-1');
      expect(loaded.items[0].prompt).toBe('first prompt');
      expect(loaded.profile).toBe('test-profile');
    });

    it('throws for non-existent file', async () => {
      await expect(loadBulkScanState(path.join(tmpDir, 'nope.json'))).rejects.toThrow();
    });

    it('rejects malformed JSON', async () => {
      const malformedPath = path.join(tmpDir, 'truncated.bulk-scan.json');
      await fs.writeFile(malformedPath, '{"version":2', 'utf-8');

      await expect(loadBulkScanState(malformedPath)).rejects.toThrow(/malformed JSON/i);
    });

    it('rejects legacy state that cannot restore prompt correlation', async () => {
      const legacyPath = path.join(tmpDir, 'legacy.bulk-scan.json');
      await fs.writeFile(
        legacyPath,
        JSON.stringify({ scanIds: ['scan-1'], profile: 'profile', promptCount: 5 }),
        'utf-8',
      );

      await expect(loadBulkScanState(legacyPath)).rejects.toThrow(/legacy|re-run bulk-scan/i);
    });

    it('rejects duplicate prompt identities', async () => {
      const duplicatePath = path.join(tmpDir, 'duplicate.bulk-scan.json');
      await fs.writeFile(
        duplicatePath,
        JSON.stringify({ ...state, items: [state.items[0], state.items[0]] }),
        'utf-8',
      );

      await expect(loadBulkScanState(duplicatePath)).rejects.toThrow(/duplicate input index/i);
    });

    it('rejects prompt entries that are not stored in input order', async () => {
      const reorderedPath = path.join(tmpDir, 'reordered.bulk-scan.json');
      await fs.writeFile(
        reorderedPath,
        JSON.stringify({
          ...state,
          items: [
            { index: 1, reqId: 1, prompt: 'second', status: 'pending' },
            { index: 0, reqId: 0, prompt: 'first', status: 'pending' },
          ],
        }),
        'utf-8',
      );

      await expect(loadBulkScanState(reorderedPath)).rejects.toThrow(/input order/i);
    });

    it('rejects malformed submitted entries before they can be resumed', async () => {
      const malformedPath = path.join(tmpDir, 'malformed.bulk-scan.json');
      await fs.writeFile(
        malformedPath,
        JSON.stringify({
          ...state,
          items: [{ index: 0, reqId: 0, prompt: 'missing receipt', status: 'submitted' }],
        }),
        'utf-8',
      );

      await expect(loadBulkScanState(malformedPath)).rejects.toThrow(/invalid bulk-scan state/i);
    });

    it('rejects a stored result whose scan ID does not match its receipt', async () => {
      const mismatchPath = path.join(tmpDir, 'mismatch.bulk-scan.json');
      await fs.writeFile(
        mismatchPath,
        JSON.stringify({
          ...state,
          items: [
            {
              ...state.items[0],
              status: 'complete',
              result: {
                index: 0,
                reqId: 0,
                prompt: 'first prompt',
                scanId: 'different-id',
                reportId: 'report-1',
                action: 'allow',
                category: 'benign',
                triggered: false,
                detections: {},
              },
            },
          ],
        }),
        'utf-8',
      );

      await expect(loadBulkScanState(mismatchPath)).rejects.toThrow(/scanId/i);
    });
  });
});
