import { afterEach, describe, expect, it, vi } from 'vitest';
import { buildProgram } from '../../../src/cli/program.js';
import { setQuiet, ui } from '../../../src/cli/renderer/ui.js';

const ANSI = /\[[0-9;]*m/g;
const stripAnsi = (s: string) => s.replace(ANSI, '');

function captureStdout() {
  const spy = vi.spyOn(console, 'log').mockImplementation(() => {});
  return { lines: () => spy.mock.calls.map((c) => stripAnsi(c.join(' '))), spy };
}

function captureStderr() {
  const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
  return { lines: () => spy.mock.calls.map((c) => stripAnsi(c.join(' '))), spy };
}

afterEach(() => {
  setQuiet(false);
  vi.restoreAllMocks();
});

describe('--quiet flag registration', () => {
  it('is a global program option', () => {
    const program = buildProgram();
    const flags = program.options.map((o) => o.long);
    expect(flags).toContain('--quiet');
  });
});

describe('quiet mode suppresses decorative output', () => {
  it('suppresses ui.status', () => {
    const err = captureStderr();
    setQuiet(true);
    ui.status('working...');
    expect(err.lines()).toHaveLength(0);
  });

  it('suppresses ui.header', () => {
    const out = captureStdout();
    setQuiet(true);
    ui.header('Title', 'subtitle');
    expect(out.lines()).toHaveLength(0);
  });

  it('suppresses ui.section', () => {
    const out = captureStdout();
    setQuiet(true);
    ui.section('Section');
    expect(out.lines()).toHaveLength(0);
  });

  it('suppresses ui.info', () => {
    const out = captureStdout();
    setQuiet(true);
    ui.info('note');
    expect(out.lines()).toHaveLength(0);
  });

  it('suppresses ui.dim', () => {
    const out = captureStdout();
    setQuiet(true);
    ui.dim('hint');
    expect(out.lines()).toHaveLength(0);
  });
});

describe('quiet mode keeps data and outcome output', () => {
  it('still prints ui.success', () => {
    const out = captureStdout();
    setQuiet(true);
    ui.success('created');
    expect(out.lines().join('\n')).toContain('✓ created');
  });

  it('still prints ui.warn', () => {
    const out = captureStdout();
    setQuiet(true);
    ui.warn('careful');
    expect(out.lines().join('\n')).toContain('⚠ careful');
  });

  it('still prints ui.error to stderr', () => {
    const err = captureStderr();
    setQuiet(true);
    ui.error('boom');
    expect(err.lines().join('\n')).toContain('✗ boom');
  });

  it('still prints ui.bullet, ui.keyValue, and ui.table', () => {
    const out = captureStdout();
    setQuiet(true);
    ui.bullet('item');
    ui.keyValue([['Key', 'value']]);
    ui.table([{ key: 'name', label: 'Name' }], [{ name: 'row1' }]);
    const text = out.lines().join('\n');
    expect(text).toContain('• item');
    expect(text).toContain('Key');
    expect(text).toContain('row1');
  });
});

describe('quiet off (default) prints everything', () => {
  it('prints ui.status and ui.header when quiet is off', () => {
    const out = captureStdout();
    const err = captureStderr();
    setQuiet(false);
    ui.status('working...');
    ui.header('Title');
    expect(err.lines().join('\n')).toContain('working...');
    expect(out.lines().join('\n')).toContain('Title');
  });
});
