import { afterEach, describe, expect, it, vi } from 'vitest';
import { ui } from '../../../src/cli/renderer/ui.js';

// biome-ignore lint/suspicious/noControlCharactersInRegex: ANSI escapes are control chars by definition
const ANSI = /\[[0-9;]*m/g;
const stripAnsi = (s: string) => s.replace(ANSI, '');

function captureStdout(): { lines: () => string[]; spy: ReturnType<typeof vi.spyOn> } {
  const spy = vi.spyOn(console, 'log').mockImplementation(() => {});
  return { lines: () => spy.mock.calls.map((c) => stripAnsi(c.join(' '))), spy };
}

function captureStderr(): { lines: () => string[]; spy: ReturnType<typeof vi.spyOn> } {
  const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
  return { lines: () => spy.mock.calls.map((c) => stripAnsi(c.join(' '))), spy };
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe('ui.header', () => {
  it('prints title with 2-space indent and optional subtitle', () => {
    const out = captureStdout();
    ui.header('Prisma AIRS — Runtime', 'profile: demo');
    const text = out.lines().join('\n');
    expect(text).toContain('  Prisma AIRS — Runtime');
    expect(text).toContain('profile: demo');
  });
});

describe('ui.keyValue', () => {
  it('aligns keys to the longest key and indents 2 spaces', () => {
    const out = captureStdout();
    ui.keyValue([
      ['Profile', 'demo'],
      ['Session ID', 'abc-123'],
    ]);
    const lines = out.lines();
    expect(lines[0]).toBe('  Profile     demo');
    expect(lines[1]).toBe('  Session ID  abc-123');
  });
});

describe('ui semantic messages', () => {
  it('uses one glyph per semantic', () => {
    const out = captureStdout();
    ui.success('created');
    ui.warn('rate limited');
    ui.info('note');
    const text = out.lines().join('\n');
    expect(text).toContain('✓ created');
    expect(text).toContain('⚠ rate limited');
    expect(text).toContain('ℹ note');
  });

  it('error goes to stderr with ✗ glyph', () => {
    const err = captureStderr();
    ui.error('failed to connect');
    expect(err.lines().join('\n')).toContain('✗ failed to connect');
  });
});

describe('ui.bullet', () => {
  it('renders the glyph for each kind', () => {
    const out = captureStdout();
    ui.bullet('item', 'neutral');
    ui.bullet('done', 'success');
    ui.bullet('skipped', 'skip');
    ui.bullet('detected', 'flag');
    const lines = out.lines();
    expect(lines[0]).toContain('• item');
    expect(lines[1]).toContain('✓ done');
    expect(lines[2]).toContain('○ skipped');
    expect(lines[3]).toContain('● detected');
  });
});

describe('ui.status', () => {
  it('writes progress lines to stderr, not stdout', () => {
    const out = captureStdout();
    const err = captureStderr();
    ui.status('Submitting async scans...');
    expect(out.lines()).toHaveLength(0);
    expect(err.lines().join('\n')).toContain('Submitting async scans...');
  });
});

describe('ui.emptyList', () => {
  it('prints the standard empty phrase', () => {
    const out = captureStdout();
    ui.emptyList('scans');
    expect(out.lines().join('\n')).toContain('No scans found');
  });
});

describe('ui.section', () => {
  it('prints a bold section label with indent', () => {
    const out = captureStdout();
    ui.section('Security Groups');
    expect(out.lines().join('\n')).toContain('  Security Groups');
  });
});

describe('ui.table', () => {
  it('delegates to the canonical box-drawing table', () => {
    const out = captureStdout();
    ui.table(
      [
        { key: 'name', label: 'Name' },
        { key: 'status', label: 'Status' },
      ],
      [{ name: 'a', status: 'ok' }],
    );
    const text = out.lines().join('\n');
    expect(text).toContain('│');
    expect(text).toContain('Name');
    expect(text).toContain('ok');
  });
});
