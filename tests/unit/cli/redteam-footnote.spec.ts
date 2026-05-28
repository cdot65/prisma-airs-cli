import { describe, expect, it } from 'vitest';
import { buildAttackListFootnote } from '../../../src/cli/renderer/redteam.js';

describe('buildAttackListFootnote', () => {
  const breakdown = [
    { severity: 'CRITICAL', successful: 97, failed: 12 },
    { severity: 'HIGH', successful: 503, failed: 98 },
  ];

  it('returns footnote when totalItems < expected for filtered severity', () => {
    const footnote = buildAttackListFootnote({
      severity: 'CRITICAL',
      totalItems: 19,
      severityBreakdown: breakdown,
    });
    expect(footnote).toBeDefined();
    expect(footnote).toContain('19');
    expect(footnote).toContain('109');
    expect(footnote).toContain('CRITICAL');
  });

  it('returns undefined when totalItems matches expected', () => {
    const footnote = buildAttackListFootnote({
      severity: 'CRITICAL',
      totalItems: 109,
      severityBreakdown: breakdown,
    });
    expect(footnote).toBeUndefined();
  });

  it('returns undefined when no severity filter set', () => {
    const footnote = buildAttackListFootnote({
      severity: undefined,
      totalItems: 19,
      severityBreakdown: breakdown,
    });
    expect(footnote).toBeUndefined();
  });

  it('returns undefined when no matching breakdown row', () => {
    const footnote = buildAttackListFootnote({
      severity: 'INFORMATIONAL',
      totalItems: 5,
      severityBreakdown: breakdown,
    });
    expect(footnote).toBeUndefined();
  });

  it('returns undefined when totalItems is unknown', () => {
    const footnote = buildAttackListFootnote({
      severity: 'CRITICAL',
      totalItems: undefined,
      severityBreakdown: breakdown,
    });
    expect(footnote).toBeUndefined();
  });
});
