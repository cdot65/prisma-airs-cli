import { describe, expect, it } from 'vitest';
import { chartFiltersFrom } from '../../../src/cli/commands/aigateway/telemetry-filters.js';
import { CliUsageError } from '../../../src/cli/renderer/index.js';

describe('chart flag syntax with the published SDK filter schema', () => {
  it('omits absent flags', () => {
    expect(chartFiltersFrom({})).toEqual({});
  });

  it('parses all filters without losing zero or fractional cents', () => {
    expect(
      chartFiltersFrom({
        traceId: 'owned',
        metadata: '{"environment":"dev"}',
        statusCodes: '200, 446',
        apiKeyIds: '11111111-1111-4111-8111-111111111111',
        aiOrgModels: 'openai__gpt-5.6-terra',
        totalUnitsMin: '0',
        totalUnitsMax: '42',
        costMin: '0',
        costMax: '1.25e-1',
      }),
    ).toEqual({
      traceId: 'owned',
      metadata: { environment: 'dev' },
      statusCodes: [200, 446],
      apiKeyIds: ['11111111-1111-4111-8111-111111111111'],
      aiOrgModels: ['openai__gpt-5.6-terra'],
      totalUnitsMin: 0,
      totalUnitsMax: 42,
      costMin: 0,
      costMax: 0.125,
    });
  });

  it.each([
    { statusCodes: '' },
    { statusCodes: '200,' },
    { statusCodes: '200,,446' },
    { statusCodes: '-1' },
    { statusCodes: '9007199254740992' },
    { statusCodes: '200junk' },
    { statusCodes: '200.0' },
    { statusCodes: '0200' },
    { apiKeyIds: 'not-a-uuid' },
    { apiKeyIds: '11111111-1111-4111-8111-111111111111,' },
    { aiOrgModels: '@openai/gpt-5.6-terra' },
    { aiOrgModels: 'openai__' },
    { aiOrgModels: 'openai__gpt-5.6-terra,' },
    { traceId: ' ' },
    { metadata: '{' },
    { metadata: 'null' },
    { metadata: '[]' },
    { metadata: '{"value":123}' },
    { metadata: '{"__proto__":"danger"}' },
    { totalUnitsMin: '-1' },
    { totalUnitsMin: '1.5' },
    { totalUnitsMin: '1e2' },
    { totalUnitsMax: '9007199254740992' },
    { totalUnitsMin: '2', totalUnitsMax: '1' },
    { costMin: '' },
    { costMin: ' ' },
    { costMin: 'Infinity' },
    { costMin: 'NaN' },
    { costMin: '0x10' },
    { costMin: '1junk' },
    { costMin: '-1' },
    { costMin: '1e999' },
    { costMin: '01' },
    { costMin: '1', costMax: '0' },
  ])('rejects invalid filters without echoing their values: %j', (flags) => {
    expect(() => chartFiltersFrom(flags)).toThrow(CliUsageError);
  });

  it('does not expose invalid metadata values in an error', () => {
    expect(() => chartFiltersFrom({ metadata: '{"value":{"secret":"do-not-echo"}}' })).toThrow(
      'Invalid chart filters',
    );
    try {
      chartFiltersFrom({ metadata: '{"value":{"secret":"do-not-echo"}}' });
    } catch (error) {
      expect(String(error)).not.toContain('do-not-echo');
    }
  });

  it('accepts inclusive equal bounds', () => {
    expect(
      chartFiltersFrom({ totalUnitsMin: '0', totalUnitsMax: '0', costMin: '0', costMax: '0' }),
    ).toEqual({ totalUnitsMin: 0, totalUnitsMax: 0, costMin: 0, costMax: 0 });
  });

  it('preserves the SDK analytics status domain, including nonstandard codes', () => {
    expect(chartFiltersFrom({ statusCodes: '0,99,600' })).toEqual({ statusCodes: [0, 99, 600] });
  });
});
