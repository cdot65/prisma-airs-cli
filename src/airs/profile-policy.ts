import type {
  AiSecurityProfile,
  ContentTypeConfiguration,
  ModelConfiguration,
  Policy,
} from '@cdot65/prisma-airs-sdk';

export class ProfilePolicyError extends Error {}

export const PROFILE_DIRECTIONS = ['prompt', 'response', 'tool-call', 'tool-response'] as const;
export type ProfileDirection = (typeof PROFILE_DIRECTIONS)[number];
export interface ProfileSelector {
  direction?: ProfileDirection;
  aiProfileIndex?: number;
}

export function validateProfileSelector(selector: ProfileSelector): void {
  if (selector.direction !== undefined && !PROFILE_DIRECTIONS.includes(selector.direction))
    throw new ProfilePolicyError(
      '--direction must be prompt, response, tool-call, or tool-response',
    );
  if (
    selector.aiProfileIndex !== undefined &&
    (!Number.isSafeInteger(selector.aiProfileIndex) || selector.aiProfileIndex < 0)
  )
    throw new ProfilePolicyError('--ai-profile-index must be a nonnegative integer');
}

export function isDirectionalProfile(profile: AiSecurityProfile): boolean {
  return (
    profile['content-type-mode'] === 'per_content_type' ||
    profile['content-type-configurations'] !== undefined
  );
}

/** Keep the AI entry and direction attached to each protection block. Includes retained legacy blocks for transfer. */
export function profileProtectionLocations(policy: Policy | undefined): Array<{
  profile: AiSecurityProfile;
  index: number;
  direction?: string;
  configuration: ContentTypeConfiguration;
  active: boolean;
}> {
  return (policy?.['ai-security-profiles'] ?? []).flatMap((profile, index) => {
    const locations: ReturnType<typeof profileProtectionLocations> = [];
    if (profile['model-configuration'])
      locations.push({
        profile,
        index,
        configuration: profile['model-configuration'],
        active: !isDirectionalProfile(profile),
      });
    for (const [direction, configuration] of Object.entries(
      profile['content-type-configurations'] ?? {},
    )) {
      if (configuration && typeof configuration === 'object' && !Array.isArray(configuration))
        locations.push({
          profile,
          index,
          direction,
          configuration: configuration as ContentTypeConfiguration,
          active: true,
        });
    }
    return locations;
  });
}

/** Select exactly one entry; never guess the first entry of a multi-model policy. */
export function selectAiProfile(policy: Policy, selector: ProfileSelector): AiSecurityProfile {
  validateProfileSelector(selector);
  const profiles = policy['ai-security-profiles'] ?? [];
  if (selector.aiProfileIndex === undefined && profiles.length > 1)
    throw new ProfilePolicyError(
      'Multiple AI profile entries: specify --ai-profile-index or use --config',
    );
  const index = selector.aiProfileIndex ?? 0;
  if (profiles.length === 0 && index === 0 && selector.aiProfileIndex === undefined) {
    const profile: AiSecurityProfile = { 'model-type': 'default' };
    policy['ai-security-profiles'] = [profile];
    return profile;
  }
  const profile = profiles[index];
  if (!profile)
    throw new ProfilePolicyError(
      '--ai-profile-index does not identify an existing AI profile entry',
    );
  return profile;
}

/** Selecting a direction on a legacy update requires a complete JSON layout replacement. */
export function selectProfileProtections(
  profile: AiSecurityProfile,
  direction?: ProfileDirection,
  allowMissingDirection = false,
): ContentTypeConfiguration {
  if (isDirectionalProfile(profile)) {
    if (!direction)
      throw new ProfilePolicyError('Directional protection edits require --direction');
    if (
      profile['content-type-mode'] !== undefined &&
      profile['content-type-mode'] !== 'per_content_type'
    )
      throw new ProfilePolicyError(
        'Unknown content-type-mode: use a complete --config replacement',
      );
    if (!profile['content-type-configurations']?.[direction] && !allowMissingDirection)
      throw new ProfilePolicyError(
        'Selected direction is absent; add it with a complete --config replacement before editing protections or topics',
      );
    profile['content-type-configurations'] ??= {};
    const configurations = profile['content-type-configurations'];
    configurations[direction] ??= {};
    return configurations[direction];
  }
  if (direction)
    throw new ProfilePolicyError(
      'A legacy profile cannot be converted by --direction; use a complete --config replacement',
    );
  if (profile['content-type-mode'] !== undefined)
    throw new ProfilePolicyError('Unknown content-type-mode: use a complete --config replacement');
  profile['model-configuration'] ??= {};
  return profile['model-configuration'];
}

export function sharedProfileSettings(profile: AiSecurityProfile): ModelConfiguration {
  profile['model-configuration'] ??= {};
  return profile['model-configuration'];
}
