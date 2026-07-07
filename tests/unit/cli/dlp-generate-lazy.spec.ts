import { describe, expect, it } from 'vitest';
import { loadGenerateCorpus } from '../../../src/cli/commands/dlp/generate.js';

describe('loadGenerateCorpus', () => {
  it('returns generateCorpus from a successful import', async () => {
    const fake = async () => ({ generateCorpus: (() => {}) as never });
    const fn = await loadGenerateCorpus(fake as never);
    expect(typeof fn).toBe('function');
  });

  it('throws an actionable install hint when an optional dep is missing', async () => {
    const err = Object.assign(new Error("Cannot find package 'sharp'"), {
      code: 'ERR_MODULE_NOT_FOUND',
    });
    const failing = async () => {
      throw err;
    };
    await expect(loadGenerateCorpus(failing as never)).rejects.toThrow(
      /pnpm add sharp pdf-lib docx piexifjs/,
    );
  });

  it('rethrows unrelated import errors unchanged', async () => {
    const failing = async () => {
      throw new Error('syntax error in module');
    };
    await expect(loadGenerateCorpus(failing as never)).rejects.toThrow('syntax error in module');
  });
});
