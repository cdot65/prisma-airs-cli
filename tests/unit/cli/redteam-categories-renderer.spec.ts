import { afterEach, describe, expect, it } from 'vitest';

let output: string[];
const originalLog = console.log;

describe('renderCategories', () => {
  afterEach(() => {
    output = [];
    console.log = originalLog;
  });

  it('prints empty state when no categories', async () => {
    output = [];
    console.log = (...args: unknown[]) => output.push(args.join(' '));
    const { renderCategories } = await import('../../../src/cli/renderer/redteam.js');
    renderCategories([]);
    expect(output.join('\n')).toContain('No categories found.');
  });

  it('prints parent and sub-category IDs inline with display names', async () => {
    output = [];
    console.log = (...args: unknown[]) => output.push(args.join(' '));
    const { renderCategories } = await import('../../../src/cli/renderer/redteam.js');
    renderCategories([
      {
        id: 'SECURITY',
        displayName: 'Security',
        description: 'Select categories for adversarial testing of security vulnerabilities',
        subCategories: [
          { id: 'JAILBREAK', displayName: 'Jailbreak', description: 'Jailbreak attempts' },
          {
            id: 'PROMPT_INJECTION',
            displayName: 'Prompt Injection',
            description: 'Direct prompt injection attacks',
          },
        ],
      },
    ]);
    const text = output.join('\n');
    expect(text).toContain('Security (SECURITY)');
    expect(text).toContain('Jailbreak (JAILBREAK)');
    expect(text).toContain('Prompt Injection (PROMPT_INJECTION)');
  });

  it('renders categories without descriptions', async () => {
    output = [];
    console.log = (...args: unknown[]) => output.push(args.join(' '));
    const { renderCategories } = await import('../../../src/cli/renderer/redteam.js');
    renderCategories([
      {
        id: 'SAFETY',
        displayName: 'Safety',
        subCategories: [{ id: 'TOXICITY', displayName: 'Toxicity' }],
      },
    ]);
    const text = output.join('\n');
    expect(text).toContain('Safety (SAFETY)');
    expect(text).toContain('Toxicity (TOXICITY)');
  });
});
