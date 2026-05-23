import { describe, expect, it } from 'vitest';
import { isAirsUrl } from '../../../src/cli/debug-logger.js';

describe('isAirsUrl', () => {
  it('matches each AIRS production host', () => {
    expect(isAirsUrl('https://api.sase.paloaltonetworks.com/foo')).toBe(true);
    expect(isAirsUrl('https://service.api.aisecurity.paloaltonetworks.com/foo')).toBe(true);
    expect(isAirsUrl('https://auth.apps.paloaltonetworks.com/oauth2/access_token')).toBe(true);
    expect(isAirsUrl('https://api.dlp.paloaltonetworks.com/v2/api/data-filtering-profiles')).toBe(
      true,
    );
  });

  it('matches regional subdomains via endsWith', () => {
    expect(isAirsUrl('https://us.api.sase.paloaltonetworks.com/foo')).toBe(true);
    expect(isAirsUrl('https://eu.api.dlp.paloaltonetworks.com/foo')).toBe(true);
  });

  it('rejects non-AIRS hosts and malformed URLs', () => {
    expect(isAirsUrl('https://example.com/foo')).toBe(false);
    expect(isAirsUrl('https://paloaltonetworks.com/foo')).toBe(false);
    expect(isAirsUrl('not-a-url')).toBe(false);
  });
});
