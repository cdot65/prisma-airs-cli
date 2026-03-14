import * as fs from 'node:fs/promises';
import * as path from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { loadBulkScanState, saveBulkScanState } from '../../../src/cli/bulk-scan-state.js';

describe('bulk-scan-state', () => {
  let tmpDir: string;

  beforeEach(async () => {
    tmpDir = await fs.mkdtemp(path.join(import.meta.dirname ?? '/tmp', 'bss-'));
  });

  afterEach(async () => {
    await fs.rm(tmpDir, { recursive: true, force: true });
  });

  describe('saveBulkScanState', () => {
    it('writes scan IDs and profile to a JSON file', async () => {
      const filePath = await saveBulkScanState(
        { scanIds: ['id-1', 'id-2'], profile: 'my-profile', promptCount: 10 },
        tmpDir,
      );

      const content = JSON.parse(await fs.readFile(filePath, 'utf-8'));
      expect(content.scanIds).toEqual(['id-1', 'id-2']);
      expect(content.profile).toBe('my-profile');
      expect(content.promptCount).toBe(10);
      expect(content.timestamp).toBeDefined();
    });

    it('creates file with .bulk-scan.json suffix', async () => {
      const filePath = await saveBulkScanState(
        { scanIds: ['id-1'], profile: 'p', promptCount: 1 },
        tmpDir,
      );

      expect(filePath).toMatch(/\.bulk-scan\.json$/);
    });

    it('creates the directory if it does not exist', async () => {
      const nestedDir = path.join(tmpDir, 'nested', 'deep');
      const filePath = await saveBulkScanState(
        { scanIds: ['id-1'], profile: 'p', promptCount: 1 },
        nestedDir,
      );

      const content = JSON.parse(await fs.readFile(filePath, 'utf-8'));
      expect(content.scanIds).toEqual(['id-1']);
    });
  });

  describe('loadBulkScanState', () => {
    it('reads saved state from a JSON file', async () => {
      const filePath = await saveBulkScanState(
        { scanIds: ['a', 'b', 'c'], profile: 'test-profile', promptCount: 15 },
        tmpDir,
      );

      const state = await loadBulkScanState(filePath);
      expect(state.scanIds).toEqual(['a', 'b', 'c']);
      expect(state.profile).toBe('test-profile');
      expect(state.promptCount).toBe(15);
    });

    it('throws for non-existent file', async () => {
      await expect(loadBulkScanState(path.join(tmpDir, 'nope.json'))).rejects.toThrow();
    });
  });
});
