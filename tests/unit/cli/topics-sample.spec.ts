import { describe, expect, it } from 'vitest';
import { buildSampleCsv } from '../../../src/cli/commands/topics-sample.js';

describe('topics-sample', () => {
  describe('buildSampleCsv', () => {
    it('returns valid CSV with header and 4 rows', () => {
      const csv = buildSampleCsv();
      const lines = csv.trim().split('\n');
      expect(lines[0]).toBe('prompt,expected,intent');
      expect(lines).toHaveLength(5); // header + 4 rows
    });

    it('includes both block and allow intent examples', () => {
      const csv = buildSampleCsv();
      expect(csv).toContain(',block');
      expect(csv).toContain(',allow');
    });

    it('includes both true and false expected values', () => {
      const csv = buildSampleCsv();
      expect(csv).toContain(',true,');
      expect(csv).toContain(',false,');
    });
  });
});
