import {
  type CreateCustomTopicRequest,
  type CreateSecurityProfileRequest,
  ManagementClient,
  type ManagementClientOptions,
  type CustomTopic as SdkCustomTopic,
} from '@cdot65/prisma-airs-sdk';
import type { ProfileTopic } from '../audit/types.js';
import type {
  ApiKeyInfo,
  ApiKeyListResult,
  ConsumptionAppListEntry,
  ConsumptionQueryOptions,
  CustomerAppConsumption,
  CustomerAppInfo,
  CustomerAppListResult,
  DeleteResponse,
  DeploymentProfileInfo,
  ManagementService,
  PaginationOptions,
  ScanLogQueryOptions,
  ScanLogQueryResult,
  SecurityProfileInfo,
  SecurityProfileListResult,
} from './types.js';

/**
 * Wraps the SDK's ManagementClient to implement our ManagementService interface.
 * OAuth2 token management, caching, and retry are handled by the SDK.
 */
export class SdkManagementService implements ManagementService {
  private client: ManagementClient;

  constructor(opts?: ManagementClientOptions) {
    this.client = getOrCreateManagementClient(opts);
  }

  async createTopic(request: CreateCustomTopicRequest): Promise<SdkCustomTopic> {
    return this.client.topics.create(request);
  }

  async updateTopic(topicId: string, request: CreateCustomTopicRequest): Promise<SdkCustomTopic> {
    return this.client.topics.update(topicId, request);
  }

  async deleteTopic(topicId: string): Promise<void> {
    await this.client.topics.delete(topicId);
  }

  async forceDeleteTopic(topicId: string, updatedBy?: string): Promise<DeleteResponse> {
    const response = await this.client.topics.forceDelete(topicId, updatedBy);
    return { message: (response as unknown as Record<string, string>).message };
  }

  async listTopics(): Promise<SdkCustomTopic[]> {
    const response = await this.client.topics.list();
    return response.custom_topics;
  }

  async getTopic(topicId: string): Promise<SdkCustomTopic> {
    const topics = await this.listTopics();
    const topic = topics.find((t) => t.topic_id === topicId);
    if (!topic) throw new Error(`Topic ${topicId} not found`);
    return topic;
  }

  async getTopicByName(topicName: string): Promise<SdkCustomTopic> {
    const topics = await this.listTopics();
    const topic = topics.find((t) => t.topic_name === topicName);
    if (!topic) throw new Error(`Topic "${topicName}" not found`);
    return topic;
  }

  /**
   * Sets a single custom topic on a profile's topic-guardrails config.
   * Delegates to {@link assignTopicsToProfile} for backward compatibility.
   */
  async assignTopicToProfile(
    profileName: string,
    topicId: string,
    topicName: string,
    action: 'allow' | 'block',
  ): Promise<void> {
    return this.assignTopicsToProfile(profileName, [{ topicId, topicName, action }]);
  }

  /**
   * Sets one or more custom topics on a profile's topic-guardrails config.
   * Replaces any existing topics — previous runs' stale topics are cleared.
   * Groups topics by action; skips empty action groups (AIRS rejects them).
   *
   * CRITICAL: Each topic entry MUST include the current `revision` number.
   * AIRS pins topic content to the revision specified in the profile — omitting
   * it defaults to revision 0 (original content), not the latest.
   */
  async assignTopicsToProfile(
    profileName: string,
    topics: Array<{ topicId: string; topicName: string; action: 'allow' | 'block' }>,
    guardrailAction?: 'allow' | 'block',
  ): Promise<void> {
    // Find profile by name
    const { ai_profiles } = await this.client.profiles.list();
    const profile = ai_profiles.find((p) => p.profile_name === profileName);
    if (!profile?.profile_id) {
      throw new Error(`Profile "${profileName}" not found`);
    }

    // Fetch current topic revisions — AIRS requires the revision field to
    // reference the correct topic content snapshot.
    const allTopics = await this.listTopics();
    const revisionMap = new Map(allTopics.map((t) => [t.topic_id, t.revision ?? 0]));

    // Deep clone the policy to mutate
    const policy = JSON.parse(JSON.stringify(profile.policy ?? {}));
    const aiProfiles = policy['ai-security-profiles'] ?? [
      { 'model-type': 'default', 'model-configuration': {} },
    ];
    const modelConfig = aiProfiles[0]?.['model-configuration'] ?? {};

    // Find or create model-protection with topic-guardrails
    const modelProtection: Record<string, unknown>[] = modelConfig['model-protection'] ?? [];
    let topicGuardrails = modelProtection.find((mp) => mp.name === 'topic-guardrails');

    if (!topicGuardrails) {
      topicGuardrails = {
        action: 'block',
        name: 'topic-guardrails',
        options: [],
        'topic-list': [],
      };
      modelProtection.push(topicGuardrails);
    }

    // Guardrail-level action controls default behavior for topic-guardrails layer.
    // 'block' = block all unless explicitly allowed (requires allow topics).
    // 'allow' = allow all unless explicitly blocked (only block topics needed).
    topicGuardrails.action = guardrailAction ?? 'block';

    // Group topics by action, build topic-list entries with revision (skip empty groups).
    const byAction = new Map<
      string,
      Array<{ topic_id: string; topic_name: string; revision: number }>
    >();
    for (const t of topics) {
      const group = byAction.get(t.action) ?? [];
      group.push({
        topic_id: t.topicId,
        topic_name: t.topicName,
        revision: revisionMap.get(t.topicId) ?? 0,
      });
      byAction.set(t.action, group);
    }

    const topicList: Array<{
      action: string;
      topic: Array<{ topic_id: string; topic_name: string; revision: number }>;
    }> = [];
    for (const [action, group] of byAction) {
      topicList.push({ action, topic: group });
    }

    topicGuardrails['topic-list'] = topicList;

    // Write back
    modelConfig['model-protection'] = modelProtection;
    aiProfiles[0]['model-configuration'] = modelConfig;
    policy['ai-security-profiles'] = aiProfiles;

    await this.client.profiles.update(profile.profile_id, {
      profile_name: profile.profile_name,
      active: profile.active,
      policy,
    });
  }

  async getProfileTopics(profileName: string): Promise<ProfileTopic[]> {
    const { ai_profiles } = await this.client.profiles.list();
    const profile = ai_profiles.find((p) => p.profile_name === profileName);
    if (!profile?.profile_id) {
      throw new Error(`Profile "${profileName}" not found`);
    }

    // Extract topic entries from profile policy
    const policy = profile.policy ?? {};
    const aiProfiles = (policy as Record<string, unknown[]>)['ai-security-profiles'] ?? [];
    const modelConfig =
      (aiProfiles[0] as Record<string, Record<string, unknown>> | undefined)?.[
        'model-configuration'
      ] ?? {};
    const modelProtection = (modelConfig['model-protection'] as Record<string, unknown>[]) ?? [];
    const topicGuardrails = modelProtection.find((mp) => mp.name === 'topic-guardrails');

    if (!topicGuardrails) return [];

    const topicList =
      (topicGuardrails['topic-list'] as Array<{
        action: string;
        topic: Array<{ topic_id: string; topic_name: string }>;
      }>) ?? [];

    // Flatten entries into topic refs with action
    const topicRefs: Array<{ topicId: string; topicName: string; action: 'allow' | 'block' }> = [];
    for (const entry of topicList) {
      for (const t of entry.topic ?? []) {
        topicRefs.push({
          topicId: t.topic_id,
          topicName: t.topic_name,
          action: entry.action as 'allow' | 'block',
        });
      }
    }

    if (topicRefs.length === 0) return [];

    // Fetch full topic details
    const allTopics = await this.listTopics();
    const topicMap = new Map(allTopics.map((t) => [t.topic_id, t]));

    return topicRefs.map((ref) => {
      const full = topicMap.get(ref.topicId);
      return {
        topicId: ref.topicId,
        topicName: ref.topicName,
        action: ref.action,
        description: full?.description ?? '',
        examples: full?.examples ?? [],
      };
    });
  }

  // -------------------------------------------------------------------------
  // Profile CRUD
  // -------------------------------------------------------------------------

  private normalizeProfile(p: Record<string, unknown>): SecurityProfileInfo {
    return {
      profileId: p.profile_id as string,
      profileName: p.profile_name as string,
      revision: p.revision as number | undefined,
      active: p.active as boolean | undefined,
      createdBy: p.created_by as string | undefined,
      updatedBy: p.updated_by as string | undefined,
      lastModifiedTs: p.last_modified_ts as string | undefined,
      policy: p.policy as Record<string, unknown> | undefined,
    };
  }

  async getProfile(profileId: string): Promise<SecurityProfileInfo> {
    const response = await this.client.profiles.get(profileId);
    return this.normalizeProfile(response as unknown as Record<string, unknown>);
  }

  async getProfileByName(profileName: string): Promise<SecurityProfileInfo> {
    const response = await this.client.profiles.getByName(profileName);
    return this.normalizeProfile(response as unknown as Record<string, unknown>);
  }

  async listProfiles(opts?: PaginationOptions): Promise<SecurityProfileListResult> {
    const response = await this.client.profiles.list(opts);
    return {
      profiles: (response.ai_profiles ?? []).map((p: Record<string, unknown>) =>
        this.normalizeProfile(p),
      ),
      nextOffset: response.next_offset,
    };
  }

  async createProfile(request: CreateSecurityProfileRequest): Promise<SecurityProfileInfo> {
    const response = await this.client.profiles.create(request);
    return this.normalizeProfile(response as unknown as Record<string, unknown>);
  }

  async updateProfile(
    profileId: string,
    request: CreateSecurityProfileRequest,
  ): Promise<SecurityProfileInfo> {
    const response = await this.client.profiles.update(profileId, request);
    return this.normalizeProfile(response as unknown as Record<string, unknown>);
  }

  async deleteProfile(profileId: string): Promise<DeleteResponse> {
    const response = await this.client.profiles.delete(profileId);
    return { message: (response as unknown as Record<string, string>).message };
  }

  async forceDeleteProfile(profileId: string, updatedBy: string): Promise<DeleteResponse> {
    const response = await this.client.profiles.forceDelete(profileId, updatedBy);
    return { message: (response as unknown as Record<string, string>).message };
  }

  // -------------------------------------------------------------------------
  // API Keys
  // -------------------------------------------------------------------------

  private normalizeApiKey(k: Record<string, unknown>): ApiKeyInfo {
    return {
      id: (k.id ?? k.api_key_id ?? '') as string,
      name: (k.name ?? k.api_key_name ?? '') as string,
      apiKey: k.api_key as string | undefined,
      last8: k.api_key_last8 as string | undefined,
      createdAt: k.created_at as string | undefined,
      expiresAt: k.expires_at as string | undefined,
    };
  }

  async listApiKeys(opts?: PaginationOptions): Promise<ApiKeyListResult> {
    const response = await this.client.apiKeys.list(opts);
    const raw = response as unknown as Record<string, unknown>;
    const keys = (raw.api_keys ?? []) as Record<string, unknown>[];
    return {
      apiKeys: keys.map((k) => this.normalizeApiKey(k)),
      nextOffset: raw.next_offset as number | undefined,
    };
  }

  async createApiKey(request: Record<string, unknown>): Promise<ApiKeyInfo> {
    const response = await this.client.apiKeys.create(request as never);
    return this.normalizeApiKey(response as unknown as Record<string, unknown>);
  }

  async regenerateApiKey(apiKeyId: string, request: Record<string, unknown>): Promise<ApiKeyInfo> {
    const response = await this.client.apiKeys.regenerate(apiKeyId, request as never);
    return this.normalizeApiKey(response as unknown as Record<string, unknown>);
  }

  async deleteApiKey(apiKeyName: string, updatedBy: string): Promise<DeleteResponse> {
    const response = await this.client.apiKeys.delete(apiKeyName, updatedBy);
    return { message: (response as unknown as Record<string, string>).message };
  }

  // -------------------------------------------------------------------------
  // Customer Apps
  // -------------------------------------------------------------------------

  private normalizeCustomerApp(a: Record<string, unknown>): CustomerAppInfo {
    return {
      id: (a.customer_app_id ?? a.id) as string | undefined,
      name: (a.app_name ?? a.name ?? '') as string,
      description: a.description as string | undefined,
      raw: a,
    };
  }

  async listCustomerApps(opts?: PaginationOptions): Promise<CustomerAppListResult> {
    const response = await this.client.customerApps.list(opts);
    const raw = response as unknown as Record<string, unknown>;
    const apps = (raw.customer_apps ?? []) as Record<string, unknown>[];
    return {
      apps: apps.map((a) => this.normalizeCustomerApp(a)),
      nextOffset: raw.next_offset as number | undefined,
    };
  }

  async getCustomerApp(appName: string): Promise<CustomerAppInfo> {
    const response = await this.client.customerApps.get(appName);
    return this.normalizeCustomerApp(response as unknown as Record<string, unknown>);
  }

  async updateCustomerApp(
    appId: string,
    request: Record<string, unknown>,
  ): Promise<CustomerAppInfo> {
    const response = await this.client.customerApps.update(appId, request as never);
    return this.normalizeCustomerApp(response as unknown as Record<string, unknown>);
  }

  async deleteCustomerApp(appName: string, updatedBy: string): Promise<CustomerAppInfo> {
    const response = await this.client.customerApps.delete(appName, updatedBy);
    return this.normalizeCustomerApp(response as unknown as Record<string, unknown>);
  }

  async listConsumptionApps(opts?: {
    limit?: number;
    offset?: number;
  }): Promise<ConsumptionAppListEntry[]> {
    // dashboard.applicationsOverview is the canonical apps-list source. customerApps.list
    // returns registered customer-apps (one per customer_appId), but the dashboard tracks
    // one bucket per distinct scan-payload metadata.app_name. Using applicationsOverview
    // surfaces every bucket - which is what the SCM UI's AI Applications view shows.
    const response = await this.client.dashboard.applicationsOverview({
      limit: opts?.limit ?? 100,
      offset: opts?.offset ?? 0,
    });
    return (response.items ?? [])
      .filter(
        (item): item is typeof item & { id: string; name: string } =>
          typeof item.id === 'string' && typeof item.name === 'string',
      )
      .map((item) => ({
        appId: item.id,
        appName: item.name,
        cloud: item.cloud ?? undefined,
        source: item.source ?? undefined,
      }));
  }

  async getCustomerAppConsumption(
    appName: string,
    opts?: ConsumptionQueryOptions,
  ): Promise<CustomerAppConsumption> {
    // The dashboard endpoints need BOTH appId and appName; the only way to resolve appId
    // from a name is via the apps-overview list, so do that first. We enumerate
    // applicationsOverview rather than customerApps.list because the dashboard buckets by
    // scan-payload metadata.app_name (not by SCM-registered app_name), so the customer-apps
    // name may not match any dashboard bucket.
    const dashboardApps = await this.listConsumptionApps({ limit: 100 });
    const target = dashboardApps.find((a) => a.appName === appName);
    const appId = target?.appId;
    if (!appId) {
      const sample = dashboardApps
        .slice(0, 5)
        .map((a) => `"${a.appName}"`)
        .join(', ');
      const more = dashboardApps.length > 5 ? `, ... (${dashboardApps.length} total)` : '';
      throw new Error(
        `Dashboard application not found: "${appName}". ` +
          `Available (as shown in SCM AI Applications view): ${sample}${more}. ` +
          `Note: the name to use is the literal value your integration sends in scan ` +
          `metadata.app_name (which may differ from the SCM application name).`,
      );
    }

    const timeInterval = opts?.timeInterval ?? 30;
    const query = { appId, appName, timeInterval } as const;

    const [overview, breakdown] = await Promise.all([
      this.client.dashboard.application(query),
      this.client.dashboard.applicationViolationBreakdown(query),
    ]);

    const ts = overview.token_stats ?? {};
    const ss = overview.session_stats ?? {};
    const detectors = (breakdown.detection_type_violation_breakdown ?? []).map((entry) => {
      const vb = entry.violation_breakdown ?? {};
      return {
        type: entry.detection_type ?? 'unknown',
        critical: vb.critical ?? 0,
        high: vb.high ?? 0,
        medium: vb.medium ?? 0,
        low: vb.low ?? 0,
        total: vb.total ?? 0,
      };
    });

    return {
      appId,
      appName,
      cloud: overview.cloud ?? undefined,
      source: overview.source ?? undefined,
      monitoringSince: overview.created_at ?? undefined,
      profiles: overview.profiles ?? [],
      tokens: {
        dailyAverage: ts.average_daily_tokens ?? undefined,
        dailyAverageScale: ts.average_daily_tokens_scale ?? undefined,
        monthlyTotal: ts.monthly_total_tokens ?? undefined,
        monthlyTotalScale: ts.monthly_total_tokens_scale ?? undefined,
      },
      sessions: {
        total: ss.total ?? 0,
        violating: ss.violating ?? 0,
      },
      detectors,
      totalViolating: breakdown.total_violating ?? 0,
    };
  }

  // -------------------------------------------------------------------------
  // Deployment Profiles (read-only)
  // -------------------------------------------------------------------------

  async listDeploymentProfiles(opts?: { unactivated?: boolean }): Promise<DeploymentProfileInfo[]> {
    const response = await this.client.deploymentProfiles.list(opts);
    const raw = response as unknown as Record<string, unknown>;
    const profiles = (raw.deployment_profiles ?? []) as Record<string, unknown>[];
    return profiles.map((p) => ({ raw: p }));
  }

  // -------------------------------------------------------------------------
  // Scan Logs
  // -------------------------------------------------------------------------

  async queryScanLogs(opts: ScanLogQueryOptions): Promise<ScanLogQueryResult> {
    const response = await this.client.scanLogs.query({
      time_interval: opts.timeInterval,
      time_unit: opts.timeUnit,
      pageNumber: opts.pageNumber,
      pageSize: opts.pageSize,
      filter: opts.filter,
      page_token: opts.pageToken,
    });
    const raw = response as unknown as Record<string, unknown>;
    const dashboard = (raw.scan_result_for_dashboard ?? {}) as Record<string, unknown>;
    const entries = (dashboard.scan_result_entries ?? raw.results ?? []) as Record<
      string,
      unknown
    >[];
    return {
      results: entries,
      pageToken: raw.page_token as string | undefined,
      raw,
    };
  }
}

let _sharedClient: ManagementClient | undefined;

/**
 * Get or lazily construct the shared ManagementClient.
 * Opts are only used on the first call; subsequent calls return the cached client.
 */
export function getOrCreateManagementClient(opts?: ManagementClientOptions): ManagementClient {
  if (!_sharedClient) {
    _sharedClient = new ManagementClient(opts);
  }
  return _sharedClient;
}

/** Test-only: reset the cached client. */
export function _resetManagementClient(): void {
  _sharedClient = undefined;
}
