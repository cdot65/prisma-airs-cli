import { describe, expect, it } from 'vitest';
import {
  assertManagementCredentials,
  assertScannerCredentials,
  hasScannerCredentials,
  missingManagementCredentials,
  settingRemedy,
} from '../../../src/config/credentials.js';
import type { ConfigContext } from '../../../src/config/loader.js';

const registryPath = '/state/tenants.json';
const named: ConfigContext = {
  path: '/state/configs/dev.json',
  selection: 'tenant',
  tenant: { name: 'dev', configPath: '/state/configs/dev.json', tsgId: '100' },
  registryPath,
};
const explicit: ConfigContext = { path: '/tmp/cfg.json', selection: 'explicit', registryPath };
const none: ConfigContext = { selection: 'none', registryPath, registered: [] };

describe('settingRemedy', () => {
  it('points a selected tenant at airs tenant set, never at environment variables', () => {
    const remedy = settingRemedy(['mgmtClientSecret'], named);
    expect(remedy).toContain('airs tenant set dev <key>');
    expect(remedy).toContain('mgmtClientSecret');
    expect(remedy).not.toContain('PANW_');
  });

  it('names the file for an explicit config path', () => {
    expect(settingRemedy(['airsApiKey'], explicit)).toBe('Add airsApiKey to /tmp/cfg.json');
  });

  it('tells a tenant-less user to create and switch', () => {
    const remedy = settingRemedy(['mgmtClientId'], none);
    expect(remedy).toContain('airs tenant create <name>');
    expect(remedy).toContain('airs tenant switch <name>');
  });
});

describe('credential predicates', () => {
  it('treats blank strings as missing', () => {
    expect(missingManagementCredentials({ mgmtClientId: ' ', mgmtTsgId: '1' })).toEqual([
      'mgmtClientId',
      'mgmtClientSecret',
    ]);
    expect(hasScannerCredentials({ airsApiToken: 'tok' })).toBe(true);
    expect(hasScannerCredentials({ airsApiKey: '' })).toBe(false);
  });

  it('asserts with a tenant-aware message', () => {
    expect(() => assertManagementCredentials({}, named)).toThrow(
      "missing mgmtClientId, mgmtClientSecret, mgmtTsgId). Run 'airs tenant set dev <key>'",
    );
    expect(() => assertScannerCredentials({}, none)).toThrow('airsApiKey or airsApiToken');
    expect(() => assertScannerCredentials({ airsApiKey: 'k' }, none)).not.toThrow();
  });
});
