import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { GatewayConfigCreateRequestSchema } from '@cdot65/prisma-airs-sdk';
import { afterEach, describe, expect, it } from 'vitest';
import {
  buildStructuredRequest,
  parseBooleanOption,
  parseCsvOption,
  parseIntegerOption,
} from '../../../src/cli/commands/aigateway/structured-input.js';

let directory: string | undefined;

afterEach(async () => {
  if (directory) await rm(directory, { recursive: true, force: true });
  directory = undefined;
});

describe('AI Gateway structured request input', () => {
  const fields = [
    { option: 'name', path: 'name' },
    { option: 'workspace', path: 'workspace_id' },
  ];

  it('builds and validates a request from named and repeatable dotted flags', async () => {
    await expect(
      buildStructuredRequest(
        {
          name: 'primary-routing',
          workspace: '11111111-1111-4111-8111-111111111111',
          set: ['config.retry.attempts=3', 'config.strategy.mode="fallback"'],
        },
        GatewayConfigCreateRequestSchema,
        fields,
      ),
    ).resolves.toEqual({
      name: 'primary-routing',
      workspace_id: '11111111-1111-4111-8111-111111111111',
      config: { retry: { attempts: 3 }, strategy: { mode: 'fallback' } },
    });
  });

  it('merges an optional file with command flags taking precedence', async () => {
    directory = await mkdtemp(join(tmpdir(), 'airs-structured-input-'));
    const file = join(directory, 'config.yaml');
    await writeFile(
      file,
      [
        'name: from-file',
        'workspace_id: 11111111-1111-4111-8111-111111111111',
        'config:',
        '  retry:',
        '    attempts: 1',
        '',
      ].join('\n'),
    );

    const request = await buildStructuredRequest(
      { file, name: 'from-flag', set: ['config.retry.attempts=4'] },
      GatewayConfigCreateRequestSchema,
      fields,
    );
    expect(request).toMatchObject({
      name: 'from-flag',
      config: { retry: { attempts: 4 } },
    });
  });

  it('supports forced strings and rejects malformed or unsafe paths', async () => {
    const base = {
      name: 'routing',
      workspace: '11111111-1111-4111-8111-111111111111',
    };
    await expect(
      buildStructuredRequest(
        { ...base, setString: ['config.provider=123'] },
        GatewayConfigCreateRequestSchema,
        fields,
      ),
    ).resolves.toMatchObject({ config: { provider: '123' } });
    await expect(
      buildStructuredRequest(
        { ...base, set: ['config.__proto__.polluted=true'] },
        GatewayConfigCreateRequestSchema,
        fields,
      ),
    ).rejects.toThrow(/unsafe/i);
    await expect(
      buildStructuredRequest(
        { ...base, set: ['missing-equals'] },
        GatewayConfigCreateRequestSchema,
        fields,
      ),
    ).rejects.toThrow(/path=value/i);
    await expect(
      buildStructuredRequest(
        { ...base, set: ['config.provider=first', 'config.provider=second'] },
        GatewayConfigCreateRequestSchema,
        fields,
      ),
    ).rejects.toThrow(/duplicate/i);
  });

  it('reports schema failures before a client can be constructed', async () => {
    await expect(
      buildStructuredRequest(
        { name: 'missing-workspace', set: ['config.retry.attempts=-1'] },
        GatewayConfigCreateRequestSchema,
        fields,
      ),
    ).rejects.toThrow(/invalid AI Gateway request/i);
  });

  it('parses reusable scalar option types strictly', () => {
    expect(parseBooleanOption('false')).toBe(false);
    expect(parseCsvOption('read, write,read')).toEqual(['read', 'write', 'read']);
    expect(parseIntegerOption('1800000')).toBe(1_800_000);
    expect(() => parseBooleanOption('yes')).toThrow(/true or false/i);
    expect(() => parseIntegerOption('1.5')).toThrow(/integer/i);
  });
});
