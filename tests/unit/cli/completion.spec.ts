import type { Command } from 'commander';
import { describe, expect, it } from 'vitest';
import {
  collectCompletionNodes,
  generateBashCompletion,
  generateFishCompletion,
  generateZshCompletion,
} from '../../../src/cli/commands/completion.js';
import { buildProgram } from '../../../src/cli/program.js';

function find(parent: Command, ...names: string[]): Command {
  let cmd = parent;
  for (const name of names) {
    const next = cmd.commands.find((c) => c.name() === name);
    if (!next) throw new Error(`command not found: ${names.join(' ')} (missing "${name}")`);
    cmd = next;
  }
  return cmd;
}

describe('completion command registration', () => {
  const program = buildProgram();

  it('registers a top-level completion command', () => {
    const completion = find(program, 'completion');
    expect(completion).toBeDefined();
    expect(completion.description()).toMatch(/completion/i);
  });
});

describe('collectCompletionNodes', () => {
  const program = buildProgram();
  const nodes = collectCompletionNodes(program);

  it('includes the root node with top-level command groups and global flags', () => {
    const root = nodes.find((n) => n.path === '');
    expect(root).toBeDefined();
    for (const word of ['runtime', 'redteam', 'model-security', 'config', 'doctor', 'completion']) {
      expect(root?.words).toContain(word);
    }
    expect(root?.words).toContain('--debug');
    expect(root?.words).toContain('--quiet');
    expect(root?.words).toContain('--help');
  });

  it('includes nested subcommand nodes with their subcommands', () => {
    const profiles = nodes.find((n) => n.path === 'runtime profiles');
    expect(profiles).toBeDefined();
    for (const word of ['list', 'get', 'create', 'update', 'delete', 'cleanup']) {
      expect(profiles?.words).toContain(word);
    }
  });

  it('includes long flags on leaf commands', () => {
    const scan = nodes.find((n) => n.path === 'runtime scan');
    expect(scan).toBeDefined();
    expect(scan?.words).toContain('--profile');
    expect(scan?.words).toContain('--response');
  });

  it('excludes hidden flags (deprecated aliases)', () => {
    const bulkScan = nodes.find((n) => n.path === 'runtime bulk-scan');
    expect(bulkScan).toBeDefined();
    expect(bulkScan?.words).toContain('--file');
    expect(bulkScan?.words).not.toContain('--input');
  });
});

describe('generateBashCompletion', () => {
  const program = buildProgram();
  const script = generateBashCompletion(program);

  it('emits a bash complete registration for airs', () => {
    expect(script).toContain('complete -F');
    expect(script).toContain('airs');
    expect(script).toContain('COMPREPLY');
  });

  it('covers nested command paths', () => {
    expect(script).toContain('runtime profiles');
    expect(script).toContain('redteam targets');
    expect(script).toContain('model-security scans');
  });

  it('contains the install snippet as a comment', () => {
    expect(script).toContain('# Install: airs completion bash');
  });
});

describe('generateZshCompletion', () => {
  const program = buildProgram();
  const script = generateZshCompletion(program);

  it('emits a zsh compdef script', () => {
    expect(script).toContain('#compdef airs');
    expect(script).toContain('compadd');
  });

  it('covers nested command paths and flags', () => {
    expect(script).toContain('runtime topics');
    expect(script).toContain('--profile');
  });

  it('contains the install snippet as a comment', () => {
    expect(script).toContain('# Install:');
    expect(script).toContain('completion zsh');
    expect(script).toContain('_airs');
  });
});

describe('generateFishCompletion', () => {
  const program = buildProgram();
  const script = generateFishCompletion(program);

  it('emits fish complete statements', () => {
    expect(script).toContain('complete -c airs');
  });

  it('covers nested command paths', () => {
    expect(script).toContain('runtime profiles');
    expect(script).toContain('redteam prompt-sets');
  });

  it('contains the install snippet as a comment', () => {
    expect(script).toContain('# Install: airs completion fish');
    expect(script).toContain('~/.config/fish/completions/airs.fish');
  });
});
