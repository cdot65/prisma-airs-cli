import { describe, expect, it } from 'vitest';
import {
  buildWorkspaceWriteRequest,
  scopeNameLooksUnrelated,
} from '../../../src/cli/commands/aigateway.js';

describe('buildWorkspaceWriteRequest', () => {
  it('collects only the flags that were passed', () => {
    expect(buildWorkspaceWriteRequest({ description: 'prod', icon: 'i' })).toEqual({
      description: 'prod',
      icon: 'i',
    });
  });

  it('returns an empty object when no writable flags are passed', () => {
    expect(buildWorkspaceWriteRequest({ output: 'json', plane: 'admin' })).toEqual({});
  });

  it('parses JSON flags for defaults and limits', () => {
    expect(
      buildWorkspaceWriteRequest({
        defaults: '{"retry":{"attempts":3}}',
        usageLimits: '[{"type":"cost","credit_limit":10000}]',
        rateLimits: '[{"type":"requests","unit":"rpm","value":100}]',
      }),
    ).toEqual({
      defaults: { retry: { attempts: 3 } },
      usageLimits: [{ type: 'cost', credit_limit: 10000 }],
      rateLimits: [{ type: 'requests', unit: 'rpm', value: 100 }],
    });
  });

  it('treats --metadata as sugar for defaults.metadata, winning over --defaults', () => {
    expect(
      buildWorkspaceWriteRequest({
        defaults: '{"metadata":{"env":"old"},"retry":1}',
        metadata: '{"env":"production"}',
      }),
    ).toEqual({
      defaults: { metadata: { env: 'production' }, retry: 1 },
    });
  });

  it('splits --users on commas and trims', () => {
    expect(buildWorkspaceWriteRequest({ users: 'a@x.io, b@x.io,' })).toEqual({
      users: ['a@x.io', 'b@x.io'],
    });
  });

  it('throws a usage error on invalid JSON', () => {
    expect(() => buildWorkspaceWriteRequest({ defaults: 'not-json' })).toThrow();
  });
});

describe('scopeNameLooksUnrelated', () => {
  it('accepts a scope containing the normalized workspace name', () => {
    expect(scopeNameLooksUnrelated('Production', 'ws_production_bx7qw0')).toBe(false);
  });

  it('flags a scope with no token overlap with the name', () => {
    expect(scopeNameLooksUnrelated('Production', 'ws_staging_zz1234')).toBe(true);
  });

  it('never flags when the name is too short to compare', () => {
    expect(scopeNameLooksUnrelated('AI', 'ws_something_else')).toBe(false);
  });
});
