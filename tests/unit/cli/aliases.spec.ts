import type { Command } from 'commander';
import { describe, expect, it } from 'vitest';
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

function walk(cmd: Command, path: string[] = []): Array<{ path: string[]; cmd: Command }> {
  const out: Array<{ path: string[]; cmd: Command }> = [];
  for (const sub of cmd.commands) {
    out.push({ path: [...path, sub.name()], cmd: sub });
    out.push(...walk(sub, [...path, sub.name()]));
  }
  return out;
}

describe('list/delete aliases', () => {
  const program = buildProgram();
  const all = walk(program);

  it('every `list` subcommand has an `ls` alias', () => {
    const lists = all.filter((e) => e.cmd.name() === 'list');
    expect(lists.length).toBeGreaterThanOrEqual(15);
    for (const { path, cmd } of lists) {
      expect(cmd.aliases(), `missing ls alias on: ${path.join(' ')}`).toContain('ls');
    }
  });

  it('every `delete` subcommand has an `rm` alias', () => {
    const deletes = all.filter((e) => e.cmd.name() === 'delete');
    expect(deletes.length).toBeGreaterThanOrEqual(10);
    for (const { path, cmd } of deletes) {
      expect(cmd.aliases(), `missing rm alias on: ${path.join(' ')}`).toContain('rm');
    }
  });

  it('spot-checks: named groups from the issue carry the aliases', () => {
    const listPaths = [
      ['runtime', 'profiles', 'list'],
      ['runtime', 'topics', 'list'],
      ['runtime', 'api-keys', 'list'],
      ['runtime', 'customer-apps', 'list'],
      ['runtime', 'deployment-profiles', 'list'],
      ['runtime', 'dlp', 'filtering-profiles', 'list'],
      ['runtime', 'dlp', 'patterns', 'list'],
      ['runtime', 'dlp', 'profiles', 'list'],
      ['runtime', 'dlp', 'dictionaries', 'list'],
      ['redteam', 'targets', 'list'],
      ['redteam', 'prompt-sets', 'list'],
      ['redteam', 'prompts', 'list'],
      ['redteam', 'properties', 'list'],
      ['model-security', 'groups', 'list'],
      ['model-security', 'rules', 'list'],
      ['model-security', 'rule-instances', 'list'],
      ['model-security', 'scans', 'list'],
    ];
    for (const path of listPaths) {
      expect(find(program, ...path).aliases(), path.join(' ')).toContain('ls');
    }

    const deletePaths = [
      ['runtime', 'profiles', 'delete'],
      ['runtime', 'topics', 'delete'],
      ['runtime', 'api-keys', 'delete'],
      ['runtime', 'customer-apps', 'delete'],
      ['runtime', 'dlp', 'patterns', 'delete'],
      ['runtime', 'dlp', 'profiles', 'delete'],
      ['runtime', 'dlp', 'dictionaries', 'delete'],
      ['redteam', 'targets', 'delete'],
      ['redteam', 'prompts', 'delete'],
      ['model-security', 'groups', 'delete'],
      ['model-security', 'labels', 'delete'],
    ];
    for (const path of deletePaths) {
      expect(find(program, ...path).aliases(), path.join(' ')).toContain('rm');
    }
  });
});

describe('destructive commands gained --force where confirmation applies', () => {
  const program = buildProgram();

  it('redteam targets delete has --force', () => {
    const flags = find(program, 'redteam', 'targets', 'delete').options.map((o) => o.long);
    expect(flags).toContain('--force');
  });

  it('runtime topics revert has --force', () => {
    const flags = find(program, 'runtime', 'topics', 'revert').options.map((o) => o.long);
    expect(flags).toContain('--force');
  });
});
