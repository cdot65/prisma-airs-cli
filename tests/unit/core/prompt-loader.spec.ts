import { describe, expect, it, vi } from 'vitest';
import { loadPrompts } from '../../../src/core/prompt-loader.js';

describe('loadPrompts', () => {
  it('parses valid CSV with header row (quoted prompts, true/false)', () => {
    const csv = `prompt,expected\n"Hello, world",true\n"Goodbye, world",false`;
    const results = loadPrompts(csv);
    expect(results).toHaveLength(2);
    expect(results[0]).toEqual({ prompt: 'Hello, world', expectedTriggered: true, category: '' });
    expect(results[1]).toEqual({ prompt: 'Goodbye, world', expectedTriggered: false, category: '' });
  });

  it('handles unquoted prompts', () => {
    const csv = `prompt,expected\nhello,true\nworld,false`;
    const results = loadPrompts(csv);
    expect(results).toHaveLength(2);
    expect(results[0]).toEqual({ prompt: 'hello', expectedTriggered: true, category: '' });
    expect(results[1]).toEqual({ prompt: 'world', expectedTriggered: false, category: '' });
  });

  it('throws if prompt column is missing', () => {
    const csv = `text,expected\nhello,true\nworld,false`;
    expect(() => loadPrompts(csv)).toThrow(/prompt/i);
  });

  it('throws if expected column is missing', () => {
    const csv = `prompt,result\nhello,true\nworld,false`;
    expect(() => loadPrompts(csv)).toThrow(/expected/i);
  });

  it('throws if no true positives', () => {
    const csv = `prompt,expected\nhello,false\nworld,false`;
    expect(() => loadPrompts(csv)).toThrow(/true.positive/i);
  });

  it('throws if no true negatives', () => {
    const csv = `prompt,expected\nhello,true\nworld,true`;
    expect(() => loadPrompts(csv)).toThrow(/true.negative/i);
  });

  it('warns on imbalanced set via callback (9 true, 1 false = 90%)', () => {
    const rows = Array.from({ length: 9 }, (_, i) => `prompt${i},true`).join('\n');
    const csv = `prompt,expected\n${rows}\nlast,false`;
    const onWarning = vi.fn();
    loadPrompts(csv, onWarning);
    expect(onWarning).toHaveBeenCalledOnce();
    expect(onWarning.mock.calls[0][0]).toMatch(/imbalanced/i);
  });

  it('handles escaped quotes in CSV ("" → ")', () => {
    const csv = `prompt,expected\n"She said ""hello""",true\nnormal,false`;
    const results = loadPrompts(csv);
    expect(results[0].prompt).toBe('She said "hello"');
  });
});
