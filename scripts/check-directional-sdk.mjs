import assert from 'node:assert/strict';
import * as sdk from '@cdot65/prisma-airs-sdk';

// A reused version number is insufficient: release only against the actual typed contract.
for (const [name, schema] of [
  ['ContentTypeConfigurationSchema', sdk.ContentTypeConfigurationSchema],
  ['ContentTypeConfigurationsSchema', sdk.ContentTypeConfigurationsSchema],
]) {
  assert.equal(
    typeof schema?.safeParse,
    'function',
    `SDK lacks ${name}; publish the directional SDK and update the registry dependency before releasing the CLI`,
  );
}
const configuration = {
  'content-type-mode': 'per_content_type',
  'model-configuration': { 'enable-full-conversation-inspection': false },
  'content-type-configurations': {
    response: {
      'model-protection': [
        { name: 'toxic-content', 'severity-by-confidence': { high: 'medium', moderate: 'low' } },
      ],
    },
  },
};
assert.deepEqual(sdk.AiSecurityProfileSchema.parse(configuration), configuration);
assert.equal(
  sdk.AiSecurityProfileSchema.safeParse({
    'content-type-configurations': { response: { 'model-protection': [{ severity: 123 }] } },
  }).success,
  false,
  'SDK must validate directional severities',
);
assert.equal(
  sdk.AiSecurityProfileSchema.safeParse({
    'model-configuration': { 'enable-full-conversation-inspection': 'false' },
  }).success,
  false,
  'SDK must validate conversation inspection',
);
assert.equal(
  sdk.SecurityProfileSchema.safeParse({ profile_name: 'test', dlp_tenant_id: 123 }).success,
  false,
  'SDK must validate DLP tenant metadata',
);
console.log('Directional security profile SDK contract verified.');
