import type { Command } from 'commander';
import { describe, expect, it } from 'vitest';
import { buildProgram } from '../../../src/cli/program.js';

function requireCommand(parent: Command, name: string): Command {
  const command = parent.commands.find(
    (candidate) => candidate.name() === name || candidate.aliases().includes(name),
  );
  if (!command) throw new Error(`Missing command: ${name}`);
  return command;
}

describe('AI Gateway command tree', () => {
  const program = buildProgram();
  const gateway = requireCommand(program, 'aigateway');

  it('exposes the canonical resource groups alphabetically', () => {
    expect(gateway.commands.map((command) => command.name()).sort()).toEqual([
      'api-keys',
      'audit-logs',
      'configs',
      'deployments',
      'guardrails',
      'integrations',
      'mcp',
      'organisations',
      'plugins',
      'providers',
      'telemetry',
      'workspaces',
    ]);
  });

  it('keeps workspace as a compatibility alias for canonical workspaces', () => {
    const workspaces = requireCommand(gateway, 'workspaces');
    expect(workspaces.aliases()).toContain('workspace');
    expect(workspaces.commands.map((command) => command.name()).sort()).toEqual([
      'archive',
      'create',
      'delete',
      'get',
      'list',
      'update',
    ]);
  });

  it('nests current and future MCP resources under mcp', () => {
    const mcp = requireCommand(gateway, 'mcp');
    const integrations = requireCommand(mcp, 'integrations');
    expect(integrations.commands.map((command) => command.name()).sort()).toEqual([
      'capabilities',
      'create',
      'delete',
      'get',
      'list',
      'metadata',
      'update',
      'workspaces',
    ]);
    expect(
      requireCommand(integrations, 'capabilities')
        .commands.map((command) => command.name())
        .sort(),
    ).toEqual(['list', 'set']);
    expect(
      requireCommand(integrations, 'workspaces')
        .commands.map((command) => command.name())
        .sort(),
    ).toEqual(['list', 'set']);
  });

  it('uses consistent collection and relationship verbs', () => {
    expect(
      requireCommand(gateway, 'configs')
        .commands.map((command) => command.name())
        .sort(),
    ).toEqual(['create', 'delete', 'get', 'list', 'update', 'versions']);
    expect(
      requireCommand(gateway, 'integrations')
        .commands.map((command) => command.name())
        .sort(),
    ).toEqual(['create', 'delete', 'get', 'list', 'models', 'update', 'workspaces']);
    expect(
      requireCommand(gateway, 'deployments')
        .commands.map((command) => command.name())
        .sort(),
    ).toEqual(['archive', 'create', 'get', 'list', 'ping', 'update']);
    expect(
      requireCommand(gateway, 'telemetry')
        .commands.map((command) => command.name())
        .sort(),
    ).toEqual([
      'cache',
      'cost',
      'error-trends',
      'errors',
      'feedback',
      'group-by',
      'latency',
      'logs',
      'requests',
      'rescued-retries',
      'tokens',
      'user-trends',
      'users',
    ]);
  });

  it('sorts the rendered AI Gateway help', () => {
    const help = gateway.helpInformation();
    const commandLines = help
      .split('\n')
      .filter((line) => /^ {2}[a-z]/.test(line) && !line.includes('--help'))
      .map((line) => line.trim().split(/\s+/)[0]);
    expect(commandLines).toEqual([...commandLines].sort());
  });

  it('makes structured flags primary and keeps --file as an optional escape hatch', () => {
    const mutations = [
      requireCommand(requireCommand(gateway, 'api-keys'), 'service').commands.find(
        (command) => command.name() === 'create',
      ),
      requireCommand(gateway, 'configs').commands.find((command) => command.name() === 'create'),
      requireCommand(gateway, 'deployments').commands.find(
        (command) => command.name() === 'create',
      ),
      requireCommand(gateway, 'guardrails').commands.find((command) => command.name() === 'create'),
      requireCommand(gateway, 'integrations').commands.find(
        (command) => command.name() === 'create',
      ),
      requireCommand(requireCommand(gateway, 'mcp'), 'integrations').commands.find(
        (command) => command.name() === 'create',
      ),
      requireCommand(gateway, 'plugins').commands.find((command) => command.name() === 'create'),
      requireCommand(gateway, 'providers').commands.find((command) => command.name() === 'create'),
    ];

    for (const command of mutations) {
      expect(command).toBeDefined();
      const file = command?.options.find((option) => option.long === '--file');
      expect(file).toBeDefined();
      expect(file?.mandatory).toBe(false);
      expect(command?.options.some((option) => option.long === '--set')).toBe(true);
      expect(command?.options.some((option) => option.long === '--set-string')).toBe(true);
    }
  });

  it('provides repeatable structured relationship flags', () => {
    const integrations = requireCommand(gateway, 'integrations');
    const modelsSet = requireCommand(requireCommand(integrations, 'models'), 'set');
    const workspacesSet = requireCommand(requireCommand(integrations, 'workspaces'), 'set');
    const mcp = requireCommand(requireCommand(gateway, 'mcp'), 'integrations');
    const capabilitiesSet = requireCommand(requireCommand(mcp, 'capabilities'), 'set');
    const mcpWorkspacesSet = requireCommand(requireCommand(mcp, 'workspaces'), 'set');

    expect(modelsSet.options.some((option) => option.long === '--model')).toBe(true);
    expect(workspacesSet.options.some((option) => option.long === '--workspace-binding')).toBe(
      true,
    );
    expect(capabilitiesSet.options.some((option) => option.long === '--capability')).toBe(true);
    expect(mcpWorkspacesSet.options.some((option) => option.long === '--workspace-binding')).toBe(
      true,
    );
  });
});
