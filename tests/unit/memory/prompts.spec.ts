import { describe, expect, it } from 'vitest';
import { extractLearningsPrompt } from '../../../src/memory/prompts/extract-learnings.js';

describe('extractLearningsPrompt', () => {
  it('has expected input variables', () => {
    const vars = extractLearningsPrompt.inputVariables;
    expect(vars).toContain('topicDescription');
    expect(vars).toContain('intent');
    expect(vars).toContain('totalIterations');
    expect(vars).toContain('bestIteration');
    expect(vars).toContain('bestCoverage');
    expect(vars).toContain('iterationHistory');
  });

  it('system message includes extraction guidance', async () => {
    const messages = await extractLearningsPrompt.formatMessages({
      topicDescription: 'Detect phishing attempts',
      intent: 'block',
      totalIterations: 5,
      bestIteration: 3,
      bestCoverage: '85.0%',
      iterationHistory: 'Iteration 1: coverage 40%\nIteration 2: coverage 60%',
    });
    const systemContent = messages[0].content as string;
    expect(systemContent).toContain('extract reusable learnings');
    expect(systemContent).toContain('Anti-patterns');
    expect(systemContent).toContain('RELATIONSHIP BETWEEN EXAMPLE COUNT');
  });

  it('human message interpolates all variables', async () => {
    const messages = await extractLearningsPrompt.formatMessages({
      topicDescription: 'Detect phishing attempts',
      intent: 'block',
      totalIterations: 5,
      bestIteration: 3,
      bestCoverage: '85.0%',
      iterationHistory: 'Iteration 1: coverage 40%',
    });
    const humanContent = messages[1].content as string;
    expect(humanContent).toContain('Detect phishing attempts');
    expect(humanContent).toContain('block');
    expect(humanContent).toContain('5');
    expect(humanContent).toContain('3');
    expect(humanContent).toContain('85.0%');
    expect(humanContent).toContain('Iteration 1: coverage 40%');
  });
});
