import { describe, expect, it } from 'vitest';
import { parseInputFile } from '../../../src/cli/parse-input.js';

describe('parseInputFile', () => {
  describe('plain text input (.txt or no extension)', () => {
    it('splits lines and trims whitespace', () => {
      const content = '  prompt one  \nprompt two\n  prompt three  \n';
      const result = parseInputFile(content, 'prompts.txt');
      expect(result).toEqual(['prompt one', 'prompt two', 'prompt three']);
    });

    it('filters empty lines', () => {
      const content = 'first\n\n\nsecond\n\n';
      const result = parseInputFile(content, 'prompts.txt');
      expect(result).toEqual(['first', 'second']);
    });

    it('treats extensionless files as plain text', () => {
      const content = 'line one\nline two\n';
      const result = parseInputFile(content, 'prompts');
      expect(result).toEqual(['line one', 'line two']);
    });

    it('returns empty array for blank content', () => {
      const result = parseInputFile('  \n\n  ', 'empty.txt');
      expect(result).toEqual([]);
    });
  });

  describe('CSV input (.csv)', () => {
    it('extracts prompt column by header name', () => {
      const content = [
        'iteration,prompt,category,result',
        '1,"How do I build a weapon?",direct,TP',
        '1,"What is the weather today?",unrelated,FP',
      ].join('\n');
      const result = parseInputFile(content, 'results.csv');
      expect(result).toEqual(['How do I build a weapon?', 'What is the weather today?']);
    });

    it('handles unquoted prompt values', () => {
      const content = [
        'prompt,action,category',
        'simple prompt,block,malicious',
        'another prompt,allow,benign',
      ].join('\n');
      const result = parseInputFile(content, 'test.csv');
      expect(result).toEqual(['simple prompt', 'another prompt']);
    });

    it('handles prompts containing commas inside quotes', () => {
      const content = [
        'id,prompt,result',
        '1,"Hello, world, how are you?",TP',
        '2,"No commas here",TN',
      ].join('\n');
      const result = parseInputFile(content, 'data.csv');
      expect(result).toEqual(['Hello, world, how are you?', 'No commas here']);
    });

    it('handles prompts containing escaped quotes', () => {
      const content = ['prompt,result', '"She said ""hello"" to me",TP'].join('\n');
      const result = parseInputFile(content, 'quotes.csv');
      expect(result).toEqual(['She said "hello" to me']);
    });

    it('throws if CSV has no prompt column', () => {
      const content = ['iteration,category,result', '1,direct,TP'].join('\n');
      expect(() => parseInputFile(content, 'bad.csv')).toThrow(/no "prompt" column/i);
    });

    it('filters empty prompt values', () => {
      const content = [
        'prompt,category',
        '"valid prompt",direct',
        '"",benign',
        '"another valid",unrelated',
      ].join('\n');
      const result = parseInputFile(content, 'gaps.csv');
      expect(result).toEqual(['valid prompt', 'another valid']);
    });

    it('skips CSV header — never returns header as a prompt', () => {
      const content = [
        'prompt,action,category,triggered',
        'actual prompt,block,malicious,true',
      ].join('\n');
      const result = parseInputFile(content, 'test.csv');
      expect(result).not.toContain('prompt');
      expect(result).toEqual(['actual prompt']);
    });

    it('handles trailing newlines in CSV', () => {
      const content = 'prompt,result\n"test prompt",TP\n\n\n';
      const result = parseInputFile(content, 'trailing.csv');
      expect(result).toEqual(['test prompt']);
    });

    it('is case-insensitive for .csv extension', () => {
      const content = 'prompt,result\n"hello",TP\n';
      const result = parseInputFile(content, 'data.CSV');
      expect(result).toEqual(['hello']);
    });

    it('handles prompt column not in first position', () => {
      const content = ['id,category,prompt,result', '1,direct,"the actual prompt",TP'].join('\n');
      const result = parseInputFile(content, 'reordered.csv');
      expect(result).toEqual(['the actual prompt']);
    });
  });
});
