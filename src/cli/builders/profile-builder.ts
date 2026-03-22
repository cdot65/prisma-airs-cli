import type { CreateSecurityProfileRequest, Policy } from '@cdot65/prisma-airs-sdk';

/**
 * Parsed CLI flag values for profile create/update commands.
 */
export interface ProfileFlags {
  // Identity
  name: string;
  active?: boolean;

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
    items.push({ name: 'toxic-content', action: flags.toxicContent });
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
      name: 'malicious-code-detection',
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

  if (flags.dlpAction) {
    const dld: Record<string, unknown> = { action: flags.dlpAction };
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
    flags.dlpAction ||
    flags.dbSecurityCreate ||
    flags.dbSecurityRead ||
    flags.dbSecurityUpdate ||
    flags.dbSecurityDelete ||
    flags.inlineTimeoutAction ||
    flags.maxInlineLatency != null ||
    flags.maskDataInStorage != null
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

  return config;
}

/** Build a full CreateSecurityProfileRequest from CLI flags (used by `create`). */
export function buildProfileRequest(flags: ProfileFlags): CreateSecurityProfileRequest {
  const request: CreateSecurityProfileRequest = {
    profile_name: flags.name,
    active: flags.active ?? true,
  };

  if (hasAnyProtectionFlag(flags)) {
    const modelConfig = buildModelConfiguration(flags);
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
  if (!hasAnyProtectionFlag(flags)) return undefined;

  const modelConfig = buildModelConfiguration(flags);
  return {
    'ai-security-profiles': [
      {
        'model-type': 'default',
        'model-configuration': modelConfig,
      },
    ],
  } as Policy;
}

/**
 * Deep-merge overrides INTO the existing policy.
 *
 * Merge rules:
 * - model-protection: keyed merge by `name`, preserves unmentioned items
 * - topic-guardrails: NEVER modified by flags — always preserved from existing
 * - app-protection: field-level overlay, preserves unmentioned fields
 * - agent-protection: keyed merge by `name`
 * - data-protection: field-level overlay; database-security merged by `name`
 * - latency: field-level overlay
 * - dlp-data-profiles: preserved unless overridden
 */
export function mergeProfilePolicy(
  existing: Record<string, unknown> | undefined,
  overrides: Policy | undefined,
): Policy {
  const base = structuredClone(existing ?? {}) as Record<string, unknown>;
  if (!overrides) return base as Policy;

  const overrideProfiles = (overrides as Record<string, unknown>)['ai-security-profiles'] as
    | Record<string, unknown>[]
    | undefined;
  if (!overrideProfiles?.length) return base as Policy;

  // Ensure base has ai-security-profiles structure
  if (!base['ai-security-profiles']) {
    base['ai-security-profiles'] = [{ 'model-type': 'default', 'model-configuration': {} }];
  }
  const baseProfiles = base['ai-security-profiles'] as Record<string, unknown>[];
  const baseConfig = (baseProfiles[0]['model-configuration'] ?? {}) as Record<string, unknown>;
  const overConfig = (overrideProfiles[0]['model-configuration'] ?? {}) as Record<string, unknown>;

  // Merge model-protection (keyed by name, topic-guardrails preserved)
  if (overConfig['model-protection']) {
    const baseMp = (baseConfig['model-protection'] ?? []) as Record<string, unknown>[];
    const overMp = overConfig['model-protection'] as Record<string, unknown>[];

    for (const overItem of overMp) {
      const existing = baseMp.find((b) => b.name === overItem.name);
      if (existing) {
        Object.assign(existing, overItem);
      } else {
        baseMp.push(overItem);
      }
    }
    baseConfig['model-protection'] = baseMp;
  }

  // Merge app-protection (field-level overlay)
  if (overConfig['app-protection']) {
    const baseAp = (baseConfig['app-protection'] ?? {}) as Record<string, unknown>;
    const overAp = overConfig['app-protection'] as Record<string, unknown>;
    Object.assign(baseAp, overAp);
    baseConfig['app-protection'] = baseAp;
  }

  // Merge agent-protection (keyed by name)
  if (overConfig['agent-protection']) {
    const baseAgp = (baseConfig['agent-protection'] ?? []) as Record<string, unknown>[];
    const overAgp = overConfig['agent-protection'] as Record<string, unknown>[];

    for (const overItem of overAgp) {
      const existing = baseAgp.find((b) => b.name === overItem.name);
      if (existing) {
        Object.assign(existing, overItem);
      } else {
        baseAgp.push(overItem);
      }
    }
    baseConfig['agent-protection'] = baseAgp;
  }

  // Merge data-protection (field-level overlay, database-security by name)
  if (overConfig['data-protection']) {
    const baseDp = (baseConfig['data-protection'] ?? {}) as Record<string, unknown>;
    const overDp = overConfig['data-protection'] as Record<string, unknown>;

    // data-leak-detection: overlay
    if (overDp['data-leak-detection']) {
      baseDp['data-leak-detection'] = overDp['data-leak-detection'];
    }

    // database-security: merge by name
    if (overDp['database-security']) {
      const baseDb = (baseDp['database-security'] ?? []) as Record<string, unknown>[];
      const overDb = overDp['database-security'] as Record<string, unknown>[];

      for (const overItem of overDb) {
        const existing = baseDb.find((b) => b.name === overItem.name);
        if (existing) {
          Object.assign(existing, overItem);
        } else {
          baseDb.push(overItem);
        }
      }
      baseDp['database-security'] = baseDb;
    }

    baseConfig['data-protection'] = baseDp;
  }

  // Merge latency (field-level overlay)
  if (overConfig.latency) {
    const baseLat = (baseConfig.latency ?? {}) as Record<string, unknown>;
    const overLat = overConfig.latency as Record<string, unknown>;
    Object.assign(baseLat, overLat);
    baseConfig.latency = baseLat;
  }

  // Merge mask-data-in-storage
  if (overConfig['mask-data-in-storage'] != null) {
    baseConfig['mask-data-in-storage'] = overConfig['mask-data-in-storage'];
  }

  baseProfiles[0]['model-configuration'] = baseConfig;
  return base as Policy;
}
