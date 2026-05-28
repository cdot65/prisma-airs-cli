import { afterEach, beforeEach, describe, expect, it } from 'vitest';

let output: string[];
const originalLog = console.log;

describe('renderAttackList', () => {
  beforeEach(() => {
    output = [];
    console.log = (...args: unknown[]) => output.push(args.join(' '));
  });

  afterEach(() => {
    console.log = originalLog;
  });

  it('prints subCategoryDisplayName when provided', async () => {
    const { renderAttackList } = await import('../../../src/cli/renderer/redteam.js');
    renderAttackList([
      {
        id: 'atk-1',
        name: undefined as unknown as string,
        severity: 'CRITICAL',
        category: 'SECURITY',
        subCategory: 'JAILBREAK',
        subCategoryDisplayName: 'Jailbreak',
        successful: false,
      },
    ]);
    const text = output.join('\n');
    expect(text).toContain('Jailbreak');
    expect(text).not.toContain('undefined');
  });

  it('falls back to subCategory raw value when display name missing', async () => {
    const { renderAttackList } = await import('../../../src/cli/renderer/redteam.js');
    renderAttackList([
      {
        id: 'atk-1',
        name: undefined as unknown as string,
        severity: 'HIGH',
        category: 'SECURITY',
        subCategory: 'JAILBREAK',
        successful: false,
      },
    ]);
    const text = output.join('\n');
    expect(text).toContain('JAILBREAK');
    expect(text).not.toContain('undefined');
  });

  it('renders BYPASSED when successful=true', async () => {
    const { renderAttackList } = await import('../../../src/cli/renderer/redteam.js');
    renderAttackList([
      {
        id: 'atk-1',
        name: undefined as unknown as string,
        severity: 'CRITICAL',
        category: 'SECURITY',
        subCategory: 'JAILBREAK',
        successful: true,
      },
    ]);
    const text = output.join('\n');
    expect(text).toContain('BYPASSED');
    expect(text).not.toContain('BLOCKED');
  });

  it('renders BLOCKED when successful=false', async () => {
    const { renderAttackList } = await import('../../../src/cli/renderer/redteam.js');
    renderAttackList([
      {
        id: 'atk-1',
        name: undefined as unknown as string,
        severity: 'HIGH',
        category: 'SECURITY',
        subCategory: 'JAILBREAK',
        successful: false,
      },
    ]);
    const text = output.join('\n');
    expect(text).toContain('BLOCKED');
    expect(text).not.toContain('BYPASSED');
  });

  it('prints footnote when passed via options', async () => {
    const { renderAttackList } = await import('../../../src/cli/renderer/redteam.js');
    renderAttackList(
      [
        {
          id: 'atk-1',
          name: undefined as unknown as string,
          severity: 'CRITICAL',
          category: 'SECURITY',
          subCategory: 'JAILBREAK',
          successful: false,
        },
      ],
      { footnote: '(showing 19 of 109 expected for severity CRITICAL — #206)' },
    );
    const text = output.join('\n');
    expect(text).toContain('19 of 109');
    expect(text).toContain('CRITICAL');
  });

  it('prints em-dash when neither subcategory field is present', async () => {
    const { renderAttackList } = await import('../../../src/cli/renderer/redteam.js');
    renderAttackList([
      {
        id: 'atk-1',
        name: undefined as unknown as string,
        severity: 'MEDIUM',
        category: 'SECURITY',
        successful: false,
      },
    ]);
    const text = output.join('\n');
    expect(text).toContain('—');
    expect(text).not.toContain('undefined');
  });
});
