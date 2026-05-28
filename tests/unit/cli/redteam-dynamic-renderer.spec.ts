import { afterEach, beforeEach, describe, expect, it } from 'vitest';

let output: string[];
const originalLog = console.log;

beforeEach(() => {
  output = [];
  console.log = (...args: unknown[]) => output.push(args.join(' '));
});

afterEach(() => {
  console.log = originalLog;
});

describe('renderDynamicReport', () => {
  it('renders header, score, asr, and goal counts', async () => {
    const { renderDynamicReport } = await import('../../../src/cli/renderer/redteam.js');
    renderDynamicReport({
      score: 75,
      asr: 0.25,
      totalGoals: 12,
      goalsAchieved: 3,
      totalStreams: 24,
      totalThreats: 5,
      reportSummary: 'Dynamic scan summary',
    });
    const text = output.join('\n');
    expect(text).toMatch(/Dynamic Scan Report/);
    expect(text).toMatch(/Score.*75/);
    expect(text).toMatch(/ASR/);
    expect(text).toMatch(/Goals/);
    expect(text).toMatch(/3/);
    expect(text).toMatch(/12/);
    expect(text).toMatch(/Threats/);
    expect(text).toMatch(/Summary/);
    expect(text).toMatch(/Dynamic scan summary/);
  });

  it('omits summary block when reportSummary is null', async () => {
    const { renderDynamicReport } = await import('../../../src/cli/renderer/redteam.js');
    renderDynamicReport({
      score: 100,
      asr: 0,
      totalGoals: 5,
      goalsAchieved: 0,
      totalStreams: 10,
      totalThreats: 0,
      reportSummary: null,
    });
    const text = output.join('\n');
    expect(text).not.toMatch(/Summary/);
  });

  it('renders with missing optional numeric fields', async () => {
    const { renderDynamicReport } = await import('../../../src/cli/renderer/redteam.js');
    renderDynamicReport({});
    const text = output.join('\n');
    expect(text).toMatch(/Dynamic Scan Report/);
  });
});
