import { describe, expect, it, vi } from 'vitest';
import { loadPrompts } from '../../../src/core/prompt-loader.js';

describe('loadPrompts', () => {
  it('parses valid CSV with header row (quoted prompts, true/false)', () => {
    const csv = `prompt,expected,intent\n"Hello, world",true,block\n"Goodbye, world",false,block`;
    const { cases } = loadPrompts(csv);
    expect(cases).toHaveLength(2);
    expect(cases[0]).toEqual({ prompt: 'Hello, world', expectedTriggered: true, category: '' });
    expect(cases[1]).toEqual({
      prompt: 'Goodbye, world',
      expectedTriggered: false,
      category: '',
    });
  });

  it('handles unquoted prompts', () => {
    const csv = `prompt,expected,intent\nhello,true,block\nworld,false,block`;
    const { cases } = loadPrompts(csv);
    expect(cases).toHaveLength(2);
    expect(cases[0]).toEqual({ prompt: 'hello', expectedTriggered: true, category: '' });
    expect(cases[1]).toEqual({ prompt: 'world', expectedTriggered: false, category: '' });
  });

  it('throws if prompt column is missing', () => {
    const csv = `text,expected,intent\nhello,true,block\nworld,false,block`;
    expect(() => loadPrompts(csv)).toThrow(/prompt/i);
  });

  it('throws if expected column is missing', () => {
    const csv = `prompt,result,intent\nhello,true,block\nworld,false,block`;
    expect(() => loadPrompts(csv)).toThrow(/expected/i);
  });

  it('throws if no true positives (after intent mapping)', () => {
    const csv = `prompt,expected,intent\nhello,false,block\nworld,false,block`;
    expect(() => loadPrompts(csv)).toThrow(/true.positive/i);
  });

  it('throws if no true negatives (after intent mapping)', () => {
    const csv = `prompt,expected,intent\nhello,true,block\nworld,true,block`;
    expect(() => loadPrompts(csv)).toThrow(/true.negative/i);
  });

  it('warns on imbalanced set via callback (9 true, 1 false = 90%)', () => {
    const rows = Array.from({ length: 9 }, (_, i) => `prompt${i},true,block`).join('\n');
    const csv = `prompt,expected,intent\n${rows}\nlast,false,block`;
    const onWarning = vi.fn();
    loadPrompts(csv, onWarning);
    expect(onWarning).toHaveBeenCalledOnce();
    expect(onWarning.mock.calls[0][0]).toMatch(/imbalanced/i);
  });

  it('handles escaped quotes in CSV ("" → ")', () => {
    const csv = `prompt,expected,intent\n"She said ""hello""",true,block\nnormal,false,block`;
    const { cases } = loadPrompts(csv);
    expect(cases[0].prompt).toBe('She said "hello"');
  });

  it('throws on empty CSV', () => {
    expect(() => loadPrompts('')).toThrow();
  });

  it('parses intent column and resolves shouldTrigger for block intent', () => {
    const csv = `prompt,expected,intent\n"weapon talk",true,block\n"cat talk",false,block`;
    const { cases, intent } = loadPrompts(csv);
    expect(intent).toBe('block');
    expect(cases).toHaveLength(2);
    expect(cases[0]).toEqual({ prompt: 'weapon talk', expectedTriggered: true, category: '' });
    expect(cases[1]).toEqual({ prompt: 'cat talk', expectedTriggered: false, category: '' });
  });

  it('flips expectedTriggered for allow intent', () => {
    const csv = `prompt,expected,intent\n"astros roster",true,allow\n"weather forecast",false,allow`;
    const { cases, intent } = loadPrompts(csv);
    expect(intent).toBe('allow');
    expect(cases[0]).toEqual({ prompt: 'astros roster', expectedTriggered: false, category: '' });
    expect(cases[1]).toEqual({ prompt: 'weather forecast', expectedTriggered: true, category: '' });
  });

  it('throws if intent column is missing', () => {
    const csv = `prompt,expected\nhello,true\nworld,false`;
    expect(() => loadPrompts(csv)).toThrow(/intent/i);
  });

  it('throws if intent values are mixed', () => {
    const csv = `prompt,expected,intent\nhello,true,block\nworld,false,allow`;
    expect(() => loadPrompts(csv)).toThrow(/same intent/i);
  });

  it('throws on invalid intent value', () => {
    const csv = `prompt,expected,intent\nhello,true,deny\nworld,false,deny`;
    expect(() => loadPrompts(csv)).toThrow(/invalid intent/i);
  });

  it('handles case-insensitive intent values', () => {
    const csv = `prompt,expected,intent\nhello,true,ALLOW\nworld,false,ALLOW`;
    const { cases, intent } = loadPrompts(csv);
    expect(intent).toBe('allow');
    expect(cases[0].expectedTriggered).toBe(false);
    expect(cases[1].expectedTriggered).toBe(true);
  });
});
