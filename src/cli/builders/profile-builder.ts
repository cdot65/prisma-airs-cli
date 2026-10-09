import {
  type CreateSecurityProfileRequest,
  type Policy,
  PolicySchema,
} from '@cdot65/prisma-airs-sdk';

import {
  type ProfileDirection,
  type ProfileSelector,
  profileProtectionLocations,
  selectAiProfile,
  selectProfileProtections,
  sharedProfileSettings,
  validateProfileSelector,
} from '../../airs/profile-policy.js';

/**
 * Parsed CLI flag values for profile create/update commands.
 */
export interface ProfileFlags {
  // Identity
  name: string;
  active?: boolean;
  direction?: ProfileDirection;
  aiProfileIndex?: number;
  enableFullConversationInspection?: boolean;

  // Model protection
  promptInjection?: string;
  toxicContent?: string;
  contextualGrounding?: string;

  // App protection
  maliciousCode?: string;
  urlAction?: string;
  allowUrlCategories?: string;
  blockUrlCategories?: string;
  alertUrlCategories?: string;

  // Agent protection
  agentSecurity?: string;

  // Data protection
  dlpAction?: string;
  dlpProfiles?: string;
  maskDataInline?: boolean;
  dbSecurityCreate?: string;
  dbSecurityRead?: string;
  dbSecurityUpdate?: string;
  dbSecurityDelete?: string;

  // Latency
  inlineTimeoutAction?: string;
  maxInlineLatency?: number;

  // Storage
  maskDataInStorage?: boolean;
}

/** Parse comma-separated string into trimmed array. */
function parseList(value: string | undefined): string[] | undefined {
  if (!value) return undefined;
  return value
    .split(',')
    .map((s) => s.trim())
    .filter(Boolean);
}

/** Build model-protection items from flags. Returns undefined if none set. */
function buildModelProtection(flags: Partial<ProfileFlags>): Record<string, unknown>[] | undefined {
  const items: Record<string, unknown>[] = [];

  if (flags.promptInjection) {
    items.push({ name: 'prompt-injection', action: flags.promptInjection });
  }
  if (flags.toxicContent) {
    // AIRS UI expects "high:<action>, moderate:<action>" format — expand bare values
    const tc = flags.toxicContent;
    const toxicAction = tc.includes(':') ? tc : `high:${tc}, moderate:${tc}`;
    items.push({ name: 'toxic-content', action: toxicAction });
  }
  if (flags.contextualGrounding) {
    items.push({ name: 'contextual-grounding', action: flags.contextualGrounding });
  }

  return items.length > 0 ? items : undefined;
}

/** Build app-protection object from flags. Returns undefined if none set. */
function buildAppProtection(flags: Partial<ProfileFlags>): Record<string, unknown> | undefined {
  const ap: Record<string, unknown> = {};
  let hasAny = false;

  if (flags.maliciousCode) {
    ap['malicious-code-protection'] = {
      name: flags.direction ? 'malicious-code' : 'malicious-code-detection',
      action: flags.maliciousCode,
    };
    hasAny = true;
  }
  if (flags.urlAction) {
    ap['url-detected-action'] = flags.urlAction;
    hasAny = true;
  }
  const allowCats = parseList(flags.allowUrlCategories);
  if (allowCats) {
    ap['allow-url-category'] = { member: allowCats };
    hasAny = true;
  }
  const blockCats = parseList(flags.blockUrlCategories);
  if (blockCats) {
    ap['block-url-category'] = { member: blockCats };
    hasAny = true;
  }
  const alertCats = parseList(flags.alertUrlCategories);
  if (alertCats) {
    ap['alert-url-category'] = { member: alertCats };
    hasAny = true;
  }

  return hasAny ? ap : undefined;
}

/** Build agent-protection array from flags. Returns undefined if none set. */
function buildAgentProtection(flags: Partial<ProfileFlags>): Record<string, unknown>[] | undefined {
  if (!flags.agentSecurity) return undefined;
  return [{ name: 'agent-security', action: flags.agentSecurity }];
}

/** Build data-protection object from flags. Returns undefined if none set. */
function buildDataProtection(flags: Partial<ProfileFlags>): Record<string, unknown> | undefined {
  const dp: Record<string, unknown> = {};
  let hasAny = false;

  if (
    flags.dlpAction !== undefined ||
    flags.dlpProfiles !== undefined ||
    flags.maskDataInline !== undefined
  ) {
    const dld: Record<string, unknown> = {};
    if (flags.dlpAction !== undefined) dld.action = flags.dlpAction;
    const profiles = parseList(flags.dlpProfiles);
    if (profiles) {
      dld.member = profiles.map((text) => ({ text }));
    }
    if (flags.maskDataInline != null) {
      dld['mask-data-inline'] = flags.maskDataInline;
    }
    dp['data-leak-detection'] = dld;
    hasAny = true;
  }

  const dbItems: Record<string, unknown>[] = [];
  if (flags.dbSecurityCreate) {
    dbItems.push({ name: 'database-security-create', action: flags.dbSecurityCreate });
  }
  if (flags.dbSecurityRead) {
    dbItems.push({ name: 'database-security-read', action: flags.dbSecurityRead });
  }
  if (flags.dbSecurityUpdate) {
    dbItems.push({ name: 'database-security-update', action: flags.dbSecurityUpdate });
  }
  if (flags.dbSecurityDelete) {
    dbItems.push({ name: 'database-security-delete', action: flags.dbSecurityDelete });
  }
  if (dbItems.length > 0) {
    dp['database-security'] = dbItems;
    hasAny = true;
  }

  return hasAny ? dp : undefined;
}

/** Build latency object from flags. Returns undefined if none set. */
function buildLatency(flags: Partial<ProfileFlags>): Record<string, unknown> | undefined {
  if (!flags.inlineTimeoutAction && flags.maxInlineLatency == null) return undefined;
  const lat: Record<string, unknown> = {};
  if (flags.inlineTimeoutAction) lat['inline-timeout-action'] = flags.inlineTimeoutAction;
  if (flags.maxInlineLatency != null) lat['max-inline-latency'] = flags.maxInlineLatency;
  return lat;
}

/** Check if any protection/config flag is set. */
function hasAnyProtectionFlag(flags: Partial<ProfileFlags>): boolean {
  return !!(
    flags.promptInjection ||
    flags.toxicContent ||
    flags.contextualGrounding ||
    flags.maliciousCode ||
    flags.urlAction ||
    flags.allowUrlCategories ||
    flags.blockUrlCategories ||
    flags.alertUrlCategories ||
    flags.agentSecurity ||
    flags.dlpAction !== undefined ||
    flags.dlpProfiles !== undefined ||
    flags.maskDataInline !== undefined ||
    flags.dbSecurityCreate ||
    flags.dbSecurityRead ||
    flags.dbSecurityUpdate ||
    flags.dbSecurityDelete ||
    flags.inlineTimeoutAction ||
    flags.maxInlineLatency != null ||
    flags.maskDataInStorage != null ||
    flags.enableFullConversationInspection != null
  );
}

/** Assemble model-configuration from flags, only including non-empty sections. */
function buildModelConfiguration(flags: Partial<ProfileFlags>): Record<string, unknown> {
  const config: Record<string, unknown> = {};

  const mp = buildModelProtection(flags);
  if (mp) config['model-protection'] = mp;

  const ap = buildAppProtection(flags);
  if (ap) config['app-protection'] = ap;

  const agp = buildAgentProtection(flags);
  if (agp) config['agent-protection'] = agp;

  const dp = buildDataProtection(flags);
  if (dp) config['data-protection'] = dp;

  const lat = buildLatency(flags);
  if (lat) config.latency = lat;

  if (flags.maskDataInStorage != null) {
    config['mask-data-in-storage'] = flags.maskDataInStorage;
  }

  if (flags.enableFullConversationInspection !== undefined)
    config['enable-full-conversation-inspection'] = flags.enableFullConversationInspection;
  return config;
}

/** Build a full CreateSecurityProfileRequest from CLI flags (used by `create`). */
export function buildProfileRequest(flags: ProfileFlags): CreateSecurityProfileRequest {
  validateProfileSelector(flags);
  if (flags.aiProfileIndex !== undefined)
    throw new Error('--ai-profile-index is only supported on updates');
  const request: CreateSecurityProfileRequest = {
    profile_name: flags.name,
    active: flags.active ?? true,
  };

  if (flags.direction) {
    if (!hasAnyProtectionFlag(flags))
      throw new Error(
        'Directional flag creation requires at least one protection or shared setting; use --config for an empty policy',
      );
    const config = buildModelConfiguration(flags);
    const shared = extractSharedSettings(config);
    request.policy = {
      'ai-security-profiles': [
        {
          'model-type': 'default',
          'content-type-mode': 'per_content_type',
          'model-configuration': shared,
          'content-type-configurations': { [flags.direction]: config },
        },
      ],
    };
  } else if (hasAnyProtectionFlag(flags)) {
    const modelConfig = buildModelConfiguration(flags);
    // AIRS UI requires these sections to exist — crashes with "is not iterable" otherwise
    if (!modelConfig['app-protection']) {
      modelConfig['app-protection'] = {
        'default-url-category': { member: ['malicious'] },
        'url-detected-action': 'block',
      };
    }
    if (!modelConfig['data-protection']) {
      modelConfig['data-protection'] = {
        'data-leak-detection': { action: '', 'mask-data-inline': false, member: null },
        'database-security': null,
      };
    }
    if (!modelConfig.latency) {
      modelConfig.latency = { 'inline-timeout-action': 'block', 'max-inline-latency': 5 };
    }
    if (modelConfig['mask-data-in-storage'] == null) {
      modelConfig['mask-data-in-storage'] = false;
    }
    request.policy = {
      'ai-security-profiles': [
        {
          'model-type': 'default',
          'model-configuration': modelConfig,
        },
      ],
    } as Policy;
  }

  return request;
}

/** Build a partial Policy from flags (used by `update` — merged into existing). */
export function buildProfileOverrides(flags: Partial<ProfileFlags>): Policy | undefined {
  validateProfileSelector(flags);
  if (!hasAnyProtectionFlag(flags)) return undefined;
  const config = buildModelConfiguration(flags);
  const maliciousCode = (config['app-protection'] as Record<string, unknown> | undefined)?.[
    'malicious-code-protection'
  ] as Record<string, unknown> | undefined;
  if (maliciousCode) delete maliciousCode.name;
  if (flags.direction) {
    const shared = extractSharedSettings(config);
    return validateOverrides({
      'ai-security-profiles': [
        {
          'model-configuration': shared,
          'content-type-configurations': { [flags.direction]: config },
        },
      ],
    });
  }
  return validateOverrides({ 'ai-security-profiles': [{ 'model-configuration': config }] });
}

/** Validate flag patches with SDK schemas, supplying required DLP action only in the validation copy. */
function validateOverrides(policy: Policy): Policy {
  const validation = structuredClone(policy);
  for (const location of profileProtectionLocations(validation)) {
    const detection = location.configuration['data-protection']?.['data-leak-detection'];
    if (detection && detection.action === undefined) detection.action = '';
    const maliciousCode = location.configuration['app-protection']?.['malicious-code-protection'];
    if (maliciousCode && maliciousCode.name === undefined)
      maliciousCode.name = 'malicious-code-detection';
  }
  PolicySchema.parse(validation);
  return policy;
}

/** Move global settings out of directional protection blocks. */
function extractSharedSettings(config: Record<string, unknown>): Record<string, unknown> {
  const shared: Record<string, unknown> = {};
  for (const key of ['latency', 'mask-data-in-storage', 'enable-full-conversation-inspection']) {
    if (Object.hasOwn(config, key)) {
      shared[key] = config[key];
      delete config[key];
    }
  }
  return shared;
}

/** Recursively overlay flags; detector arrays merge by name, retaining nested metadata. */
function mergeSettings(base: Record<string, unknown>, patch: Record<string, unknown>): void {
  for (const [key, value] of Object.entries(patch)) {
    if (
      Array.isArray(value) &&
      ['model-protection', 'agent-protection', 'database-security'].includes(key)
    ) {
      const items = (base[key] ?? []) as Record<string, unknown>[];
      for (const item of value as Record<string, unknown>[]) {
        const matches = items.filter((entry) => entry.name === item.name);
        if (matches.length > 1)
          throw new Error(`Ambiguous detector identity: ${String(item.name)}`);
        if (matches[0]) mergeSettings(matches[0], item);
        else items.push(structuredClone(item));
      }
      base[key] = items;
    } else if (value && typeof value === 'object' && !Array.isArray(value)) {
      const existing = base[key];
      const target =
        existing && typeof existing === 'object' && !Array.isArray(existing)
          ? (existing as Record<string, unknown>)
          : {};
      mergeSettings(target, value as Record<string, unknown>);
      if (key === 'malicious-code-protection' && target.name === undefined)
        target.name = 'malicious-code';
      base[key] = target;
    } else base[key] = structuredClone(value);
  }
}

/** Merge one explicitly selected entry/direction and its shared settings, preserving all others. */
export function mergeProfilePolicy(
  existing: Record<string, unknown> | undefined,
  overrides: Policy | undefined,
  selector: ProfileSelector = {},
): Policy {
  validateProfileSelector(selector);
  const base = structuredClone(existing ?? {}) as Policy;
  if (!overrides) {
    if (selector.direction !== undefined || selector.aiProfileIndex !== undefined) {
      const profile = selectAiProfile(base, selector);
      if (selector.direction !== undefined)
        selectProfileProtections(structuredClone(profile), selector.direction, true);
    }
    return base;
  }
  const patch = overrides['ai-security-profiles']?.[0];
  if (!patch) return base;
  const profile = selectAiProfile(base, selector);
  const configuration = structuredClone(patch['model-configuration'] ?? {});
  const shared = extractSharedSettings(configuration);
  const directions = Object.keys(patch['content-type-configurations'] ?? {});
  const direction = directions[0] as ProfileDirection | undefined;
  if (directions.length > 1) throw new Error('Flag overrides must select one direction');
  // An explicit selector also validates layout for global-only writes.
  if (direction || selector.direction || Object.keys(configuration).length) {
    const protectionPatch = direction
      ? (patch['content-type-configurations']?.[direction] ?? {})
      : configuration;
    const target = selectProfileProtections(
      Object.keys(protectionPatch).length ? profile : structuredClone(profile),
      direction ?? selector.direction,
      Object.keys(protectionPatch).length === 0,
    );
    mergeSettings(
      target,
      direction ? (patch['content-type-configurations']?.[direction] ?? {}) : configuration,
    );
  }
  if (Object.keys(shared).length) mergeSettings(sharedProfileSettings(profile), shared);
  return base;
}
