import { describe, expect, it } from 'vitest';
import { analyzeResultsPrompt } from '../../../src/llm/prompts/analyze-results.js';
import { generateTestsPrompt } from '../../../src/llm/prompts/generate-tests.js';
import {
  buildSeedExamplesSection,
  generateTopicPrompt,
} from '../../../src/llm/prompts/generate-topic.js';
import { improveTopicPrompt } from '../../../src/llm/prompts/improve-topic.js';
import { simplifyTopicPrompt } from '../../../src/llm/prompts/simplify-topic.js';

describe('buildSeedExamplesSection', () => {
  it('returns empty string for undefined', () => {
    expect(buildSeedExamplesSection(undefined)).toBe('');
  });

  it('returns empty string for empty array', () => {
    expect(buildSeedExamplesSection([])).toBe('');
  });

  it('returns numbered list for seeds', () => {
    const result = buildSeedExamplesSection(['alpha', 'beta']);
    expect(result).toContain('1. alpha');
    expect(result).toContain('2. beta');
    expect(result).toContain('Seed examples');
  });
});

describe('prompt templates', () => {
  it('generateTopicPrompt has expected input variables', () => {
    const vars = generateTopicPrompt.inputVariables;
    expect(vars).toContain('topicDescription');
    expect(vars).toContain('intent');
    expect(vars).toContain('seedExamplesSection');
    expect(vars).toContain('memorySection');
  });

  it('generateTestsPrompt has expected input variables', () => {
    const vars = generateTestsPrompt.inputVariables;
    expect(vars).toContain('topicName');
    expect(vars).toContain('topicDescription');
    expect(vars).toContain('topicExamples');
    expect(vars).toContain('exampleCount');
    expect(vars).toContain('intent');
    expect(vars).toContain('memorySection');
  });

  it('analyzeResultsPrompt has expected input variables', () => {
    const vars = analyzeResultsPrompt.inputVariables;
    expect(vars).toContain('topicName');
    expect(vars).toContain('topicDescription');
    expect(vars).toContain('exampleCount');
    expect(vars).toContain('falsePositives');
    expect(vars).toContain('falseNegatives');
    expect(vars).toContain('intent');
    expect(vars).toContain('memorySection');
  });

  it('improveTopicPrompt has expected input variables', () => {
    const vars = improveTopicPrompt.inputVariables;
    expect(vars).toContain('currentName');
    expect(vars).toContain('currentDescription');
    expect(vars).toContain('currentExamples');
    expect(vars).toContain('exampleCount');
    expect(vars).toContain('iteration');
    expect(vars).toContain('intent');
    expect(vars).toContain('memorySection');
    expect(vars).toContain('bestCoverage');
    expect(vars).toContain('bestIteration');
    expect(vars).toContain('bestTopicSection');
  });

  it('improveTopicPrompt system message includes platform constraint warning', async () => {
    const messages = await improveTopicPrompt.formatMessages({
      currentName: 'Test',
      currentDescription: 'Test desc',
      currentExamples: 'ex1, ex2',
      exampleCount: 2,
      iteration: 2,
      coverage: '50.0%',
      targetCoverage: '90.0%',
      tpr: '50.0%',
      tnr: '50.0%',
      accuracy: '50.0%',
      bestCoverage: '60.0%',
      bestIteration: 1,
      bestTopicSection: '',
      analysisSummary: 'summary',
      fpPatterns: 'None',
      fnPatterns: 'None',
      specificFPs: 'None',
      specificFNs: 'None',
      suggestions: 'suggestion',
      intent: 'block',
      memorySection: '',
    });
    const systemContent = messages[0].content as string;
    expect(systemContent).toContain('CRITICAL PLATFORM CONSTRAINT');
    expect(systemContent).toContain('Exclusion clauses');
    expect(systemContent).toContain('SHORTER descriptions');
  });

  it('improveTopicPrompt includes best context in human message', async () => {
    const messages = await improveTopicPrompt.formatMessages({
      currentName: 'Test',
      currentDescription: 'Test desc',
      currentExamples: 'ex1, ex2',
      exampleCount: 2,
      iteration: 3,
      coverage: '50.0%',
      targetCoverage: '90.0%',
      tpr: '50.0%',
      tnr: '50.0%',
      accuracy: '50.0%',
      bestCoverage: '70.0%',
      bestIteration: 2,
      bestTopicSection: '',
      analysisSummary: 'summary',
      fpPatterns: 'None',
      fnPatterns: 'None',
      specificFPs: 'None',
      specificFNs: 'None',
      suggestions: 'suggestion',
      intent: 'block',
      memorySection: '',
    });
    const humanContent = messages[1].content as string;
    expect(humanContent).toContain('Best so far: 70.0% coverage at iteration 2');
  });

  it('improveTopicPrompt includes regression warning when coverage regresses', async () => {
    const regressionWarning = `REGRESSION WARNING: Coverage has dropped from 70.0% (iteration 2) to 50.0%.
The best-performing definition was:
  Description: Best desc
  Examples: ex1, ex2
Consider reverting toward this simpler definition rather than adding more specificity.`;

    const messages = await improveTopicPrompt.formatMessages({
      currentName: 'Test',
      currentDescription: 'Test desc',
      currentExamples: 'ex1, ex2',
      exampleCount: 2,
      iteration: 3,
      coverage: '50.0%',
      targetCoverage: '90.0%',
      tpr: '50.0%',
      tnr: '50.0%',
      accuracy: '50.0%',
      bestCoverage: '70.0%',
      bestIteration: 2,
      bestTopicSection: regressionWarning,
      analysisSummary: 'summary',
      fpPatterns: 'None',
      fnPatterns: 'None',
      specificFPs: 'None',
      specificFNs: 'None',
      suggestions: 'suggestion',
      intent: 'block',
      memorySection: '',
    });
    const humanContent = messages[1].content as string;
    expect(humanContent).toContain('REGRESSION WARNING');
    expect(humanContent).toContain('Best desc');
  });

  it('simplifyTopicPrompt has expected input variables', () => {
    const vars = simplifyTopicPrompt.inputVariables;
    expect(vars).toContain('currentName');
    expect(vars).toContain('currentDescription');
    expect(vars).toContain('currentExamples');
    expect(vars).toContain('bestCoverage');
    expect(vars).toContain('bestDescription');
    expect(vars).toContain('bestExamples');
    expect(vars).toContain('coverage');
    expect(vars).toContain('tpr');
    expect(vars).toContain('tnr');
    expect(vars).toContain('intent');
    expect(vars).toContain('memorySection');
  });

  it('simplifyTopicPrompt system message includes anti-specificity guidance', async () => {
    const messages = await simplifyTopicPrompt.formatMessages({
      currentName: 'Test',
      currentDescription: 'Long over-refined description with not X and excludes Y',
      currentExamples: 'ex1, ex2, ex3',
      bestCoverage: '66.0%',
      bestDescription: 'Short clear description',
      bestExamples: 'ex1, ex2',
      coverage: '40.0%',
      tpr: '80.0%',
      tnr: '40.0%',
      intent: 'allow',
      memorySection: '',
    });
    const systemContent = messages[0].content as string;
    expect(systemContent).toContain('Do NOT add qualifiers');
    expect(systemContent).toContain('UNDER 80 characters');
    expect(systemContent).toContain('GOOD vs BAD');
    expect(systemContent).toContain('Cooking recipes for specific dishes');
  });

  it('simplifyTopicPrompt human message leads with best-performing definition', async () => {
    const messages = await simplifyTopicPrompt.formatMessages({
      currentName: 'Test',
      currentDescription: 'Over-refined version',
      currentExamples: 'ex1, ex2, ex3',
      bestCoverage: '66.0%',
      bestDescription: 'Short best description',
      bestExamples: 'ex1, ex2',
      coverage: '40.0%',
      tpr: '80.0%',
      tnr: '40.0%',
      intent: 'allow',
      memorySection: '',
    });
    const humanContent = messages[1].content as string;
    // Best definition should appear BEFORE current definition
    const bestIdx = humanContent.indexOf('Short best description');
    const currentIdx = humanContent.indexOf('Over-refined version');
    expect(bestIdx).toBeGreaterThan(-1);
    expect(currentIdx).toBeGreaterThan(-1);
    expect(bestIdx).toBeLessThan(currentIdx);
  });

  it('improveTopicPrompt omits regression warning when coverage is at or above best', async () => {
    const messages = await improveTopicPrompt.formatMessages({
      currentName: 'Test',
      currentDescription: 'Test desc',
      currentExamples: 'ex1, ex2',
      exampleCount: 2,
      iteration: 3,
      coverage: '70.0%',
      targetCoverage: '90.0%',
      tpr: '70.0%',
      tnr: '70.0%',
      accuracy: '70.0%',
      bestCoverage: '70.0%',
      bestIteration: 2,
      bestTopicSection: '',
      analysisSummary: 'summary',
      fpPatterns: 'None',
      fnPatterns: 'None',
      specificFPs: 'None',
      specificFNs: 'None',
      suggestions: 'suggestion',
      intent: 'block',
      memorySection: '',
    });
    const humanContent = messages[1].content as string;
    expect(humanContent).not.toContain('REGRESSION WARNING');
  });
});
