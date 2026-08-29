import { RedTeamClient, type RedTeamClientOptions } from '@cdot65/prisma-airs-sdk';
import type {
  EulaContent,
  EulaStatus,
  InstanceDetail,
  InstanceRequest,
  InstanceResponse,
  RedTeamAdapterCreateRequest,
  RedTeamAdapterDetail,
  RedTeamAdapterListItem,
  RedTeamAdapterListOptions,
  RedTeamAdapterUpdateOverrides,
  RedTeamAdapterValidateRequest,
  RedTeamAdapterValidationResult,
  RedTeamAdapterVar,
  RedTeamAttack,
  RedTeamCategory,
  RedTeamChannel,
  RedTeamChannelCreateRequest,
  RedTeamChannelListOptions,
  RedTeamChannelStats,
  RedTeamChannelUpdateRequest,
  RedTeamCustomAttack,
  RedTeamCustomReport,
  RedTeamDynamicReport,
  RedTeamErrorLog,
  RedTeamJob,
  RedTeamLanguages,
  RedTeamService,
  RedTeamStaticReport,
  RedTeamTarget,
  RedTeamTargetCreateRequest,
  RedTeamTargetDetail,
  RedTeamTargetUpdateRequest,
  RegistryCredentials,
  TargetAuthValidationRequest,
  TargetAuthValidationResult,
  TargetOperationOptions,
} from './types.js';

const TERMINAL_STATUSES = new Set(['COMPLETED', 'PARTIALLY_COMPLETE', 'FAILED', 'ABORTED']);

export const DEFAULT_DYNAMIC_BREADTH = 6;
export const DEFAULT_DYNAMIC_DEPTH = 10;

/** Normalize an SDK job response into a RedTeamJob. */
function normalizeJob(raw: Record<string, unknown>): RedTeamJob {
  const target = raw.target as Record<string, unknown> | undefined;
  return {
    uuid: raw.uuid as string,
    name: raw.name as string,
    status: raw.status as string,
    jobType: raw.job_type as string,
    targetId: raw.target_id as string,
    targetName: target?.name as string | undefined,
    score: raw.score as number | null | undefined,
    asr: raw.asr as number | null | undefined,
    total: raw.total as number | null | undefined,
    completed: raw.completed as number | null | undefined,
    createdAt: raw.created_at as string | null | undefined,
  };
}

/**
 * Map known Mustache-style severity tokens in static-report summaries to
 * readable strings. Upstream's report renderer occasionally ships placeholders
 * like `{{HIGH_RISK}}` un-interpolated. Unknown `{{...}}` tokens are left
 * intact so future upstream additions remain visible instead of silently
 * stripped.
 */
const REPORT_SUMMARY_TOKENS: Record<string, string> = {
  '{{CRITICAL_RISK}}': 'critical risk',
  '{{HIGH_RISK}}': 'high risk',
  '{{MEDIUM_RISK}}': 'medium risk',
  '{{LOW_RISK}}': 'low risk',
  '{{INFORMATIONAL_RISK}}': 'informational risk',
};

export function interpolateReportSummary<T extends string | null | undefined>(summary: T): T {
  if (summary == null || summary === '') return summary;
  let out = summary as string;
  for (const [token, replacement] of Object.entries(REPORT_SUMMARY_TOKENS)) {
    if (out.includes(token)) out = out.split(token).join(replacement);
  }
  return out as T;
}

/**
 * Drop noisy "you didn't opt in" fields from `target_metadata` when the
 * corresponding feature is disabled. `multi_turn_error_message` always comes
 * back populated, but it's only a real error when `multi_turn === true`.
 */
export function sanitizeTargetMetadata<T extends Record<string, unknown> | undefined>(
  metadata: T,
): T {
  if (!metadata) return metadata;
  if (metadata.multi_turn === false && 'multi_turn_error_message' in metadata) {
    const { multi_turn_error_message: _drop, ...rest } = metadata;
    return rest as T;
  }
  return metadata;
}

/** Normalize an SDK network broker channel into a RedTeamChannel. */
function normalizeChannel(raw: Record<string, unknown>): RedTeamChannel {
  return {
    uuid: raw.uuid as string | undefined,
    name: raw.name as string | null | undefined,
    description: raw.description as string | null | undefined,
    status: raw.status as string | null | undefined,
    addedBy: raw.added_by as string | null | undefined,
    createdAt: raw.created_at as string | null | undefined,
    updatedAt: raw.updated_at as string | null | undefined,
    lastOnlineAt: raw.last_online_at as string | null | undefined,
    connectedClientsCount: raw.connected_clients_count as number | null | undefined,
    outdatedClientsCount: raw.outdated_clients_count as number | null | undefined,
    features: raw.features as Record<string, boolean> | null | undefined,
  };
}

/** Normalize an SDK target-profile error log into a RedTeamErrorLog. */
function normalizeErrorLog(raw: Record<string, unknown>): RedTeamErrorLog {
  return {
    createdAt: raw.created_at as string,
    updatedAt: raw.updated_at as string,
    jobId: raw.job_id as string | null | undefined,
    targetId: raw.target_id as string | null | undefined,
    targetVersion: raw.target_version as number | null | undefined,
    attackId: raw.attack_id as string | null | undefined,
    errorType: raw.error_type as string | null | undefined,
    errorSource: raw.error_source as string | null | undefined,
    errorMessage: raw.error_message as string | null | undefined,
    targetObject: raw.target_object as Record<string, unknown> | null | undefined,
    extraInfo: raw.extra_info as Record<string, unknown> | null | undefined,
    version: raw.version as number | undefined,
  };
}

/** Normalize an SDK target response into a RedTeamTargetDetail. */
function normalizeTargetDetail(raw: Record<string, unknown>): RedTeamTargetDetail {
  return {
    uuid: raw.uuid as string,
    name: raw.name as string,
    status: raw.status as string,
    targetType: raw.target_type as string | undefined,
    active: raw.active as boolean,
    connectionType: raw.connection_type as string | null | undefined,
    apiEndpointType: raw.api_endpoint_type as string | null | undefined,
    responseMode: raw.response_mode as string | null | undefined,
    authType: raw.auth_type as string | null | undefined,
    authConfig: raw.auth_config as Record<string, unknown> | null | undefined,
    networkBrokerChannelUuid: raw.network_broker_channel_uuid as string | null | undefined,
    sessionSupported: raw.session_supported as boolean | undefined,
    extraInfo: raw.extra_info as Record<string, unknown> | null | undefined,
    description: raw.description as string | null | undefined,
    connectionParams: raw.connection_params as Record<string, unknown> | undefined,
    background: raw.target_background as RedTeamTargetDetail['background'],
    additionalContext: raw.additional_context as RedTeamTargetDetail['additionalContext'],
    metadata: sanitizeTargetMetadata(raw.target_metadata as RedTeamTargetDetail['metadata']),
  };
}

/**
 * Wraps the SDK's RedTeamClient to implement RedTeamService.
 * Provides scan creation, status polling, report retrieval, and target/category listing.
 */
/** Normalize an SDK adapter list row. */
function normalizeAdapterListItem(raw: Record<string, unknown>): RedTeamAdapterListItem {
  return {
    uuid: raw.uuid as string,
    name: raw.name as string,
    status: raw.status as string,
    createdAt: raw.created_at as string | undefined,
    updatedAt: raw.updated_at as string | undefined,
    createdByUserId: raw.created_by_user_id as string | null | undefined,
    targetCount: raw.target_count as number | null | undefined,
  };
}

/** Normalize an SDK adapter variable, keeping the redaction flag. */
function normalizeAdapterVar(raw: Record<string, unknown>): RedTeamAdapterVar {
  return {
    key: raw.key as string,
    value: raw.value as string | null | undefined,
    type: raw.type as 'VAR' | 'SECRET',
    isRedacted: raw.is_redacted as boolean | undefined,
  };
}

/** Normalize a full SDK adapter record. */
function normalizeAdapterDetail(raw: Record<string, unknown>): RedTeamAdapterDetail {
  return {
    uuid: raw.uuid as string,
    tsgId: raw.tsg_id as string | undefined,
    name: raw.name as string,
    scriptB64: raw.script_b64 as string,
    status: raw.status as string,
    description: raw.description as string | null | undefined,
    networkBrokerChannelUuid: raw.network_broker_channel_uuid as string | null | undefined,
    variables: ((raw.variables ?? []) as Array<Record<string, unknown>>).map(normalizeAdapterVar),
    targetCount: raw.target_count as number | null | undefined,
    createdAt: raw.created_at as string | null | undefined,
    updatedAt: raw.updated_at as string | null | undefined,
    createdByUserId: raw.created_by_user_id as string | null | undefined,
    updatedByUserId: raw.updated_by_user_id as string | null | undefined,
  };
}

/**
 * Map stored variables into a resend-safe array: a redacted (or valueless)
 * variable becomes `value: null`, which upstream reads as "keep the stored
 * value". Used so update's full-replacement PUT never silently wipes
 * variables, and so validate can send the full key set it requires.
 */
export function preserveVariablesForUpdate(variables: RedTeamAdapterVar[]): Array<{
  key: string;
  value: string | null;
  type: 'VAR' | 'SECRET';
}> {
  return variables.map((v) => ({
    key: v.key,
    value: v.isRedacted ? null : (v.value ?? null),
    type: v.type,
  }));
}

/** Map CLI-side adapter variables to the SDK wire shape (drops isRedacted). */
function toWireVariables(
  variables: RedTeamAdapterVar[],
): Array<{ key: string; value?: string | null; type: 'VAR' | 'SECRET' }> {
  return variables.map((v) => ({ key: v.key, value: v.value, type: v.type }));
}

export class SdkRedTeamService implements RedTeamService {
  private client: RedTeamClient;

  constructor(opts?: RedTeamClientOptions) {
    this.client = new RedTeamClient(opts);
  }

  async getEulaContent(): Promise<EulaContent> {
    const response = (await this.client.eula.getContent()) as Record<string, unknown>;
    return { content: response.content as string };
  }

  async getEulaStatus(): Promise<EulaStatus> {
    const raw = (await this.client.eula.getStatus()) as Record<string, unknown>;
    return {
      isAccepted: raw.is_accepted as boolean,
      acceptedAt: raw.accepted_at as string | undefined,
      acceptedByUserId: raw.accepted_by_user_id as string | undefined,
    };
  }

  async acceptEula(eulaContent: string): Promise<EulaStatus> {
    const raw = (await this.client.eula.accept({
      eula_content: eulaContent,
      accepted_at: new Date().toISOString(),
    })) as Record<string, unknown>;
    return {
      isAccepted: raw.is_accepted as boolean,
      acceptedAt: raw.accepted_at as string | undefined,
      acceptedByUserId: raw.accepted_by_user_id as string | undefined,
    };
  }

  async createInstance(request: InstanceRequest): Promise<InstanceResponse> {
    const raw = (await this.client.instances.createInstance({
      tsg_id: request.tsgId,
      tenant_id: request.tenantId,
      app_id: request.appId,
      region: request.region,
    })) as Record<string, unknown>;
    return {
      tsgId: raw.tsg_id as string,
      tenantId: raw.tenant_id as string | undefined,
      appId: raw.app_id as string | undefined,
      isSuccess: raw.is_success as boolean | undefined,
    };
  }

  async getInstance(tenantId: string): Promise<InstanceDetail> {
    const raw = (await this.client.instances.getInstance(tenantId)) as Record<string, unknown>;
    return {
      tsgId: raw.tsg_id as string,
      tenantId: raw.tenant_id as string,
      appId: raw.app_id as string,
      region: raw.region as string,
    };
  }

  async updateInstance(tenantId: string, request: InstanceRequest): Promise<InstanceResponse> {
    const raw = (await this.client.instances.updateInstance(tenantId, {
      tsg_id: request.tsgId,
      tenant_id: request.tenantId,
      app_id: request.appId,
      region: request.region,
    })) as Record<string, unknown>;
    return {
      tsgId: raw.tsg_id as string,
      tenantId: raw.tenant_id as string | undefined,
      appId: raw.app_id as string | undefined,
      isSuccess: raw.is_success as boolean | undefined,
    };
  }

  async deleteInstance(tenantId: string): Promise<InstanceResponse> {
    const raw = (await this.client.instances.deleteInstance(tenantId)) as Record<string, unknown>;
    return {
      tsgId: raw.tsg_id as string,
      tenantId: raw.tenant_id as string | undefined,
      appId: raw.app_id as string | undefined,
      isSuccess: raw.is_success as boolean | undefined,
    };
  }

  async createDevices(
    tenantId: string,
    request: Record<string, unknown>,
  ): Promise<Record<string, unknown>> {
    return (await this.client.instances.createDevices(tenantId, request as never)) as Record<
      string,
      unknown
    >;
  }

  async updateDevices(
    tenantId: string,
    request: Record<string, unknown>,
  ): Promise<Record<string, unknown>> {
    return (await this.client.instances.updateDevices(tenantId, request as never)) as Record<
      string,
      unknown
    >;
  }

  async deleteDevices(tenantId: string, serialNumbers: string): Promise<Record<string, unknown>> {
    return (await this.client.instances.deleteDevices(tenantId, serialNumbers)) as Record<
      string,
      unknown
    >;
  }

  async getRegistryCredentials(): Promise<RegistryCredentials> {
    const raw = (await this.client.instances.getRegistryCredentials()) as Record<string, unknown>;
    return {
      token: raw.token as string,
      expiry: raw.expiry as string,
    };
  }

  async validateTargetAuth(
    request: TargetAuthValidationRequest,
  ): Promise<TargetAuthValidationResult> {
    const sdkRequest: Record<string, unknown> = {
      auth_type: request.authType,
      auth_config: request.authConfig,
    };
    if (request.targetId) sdkRequest.target_id = request.targetId;
    const raw = (await this.client.targets.validateAuth(sdkRequest as never)) as Record<
      string,
      unknown
    >;
    return {
      validated: raw.validated as boolean,
      tokenPreview: raw.token_preview as string | undefined,
      expiresIn: raw.expires_in as number | undefined,
    };
  }

  async getTargetMetadata(): Promise<Record<string, unknown>> {
    return (await this.client.targets.getTargetMetadata()) as Record<string, unknown>;
  }

  async getTargetTemplates(): Promise<Record<string, unknown>> {
    return (await this.client.targets.getTargetTemplates()) as Record<string, unknown>;
  }

  async listTargets(): Promise<RedTeamTarget[]> {
    const targets = await this.client.targets.listAll({ limit: 100 });
    return targets.map((target) => ({
      uuid: target.uuid,
      name: target.name,
      status: String(target.status ?? ''),
      targetType: target.target_type == null ? undefined : String(target.target_type),
      active: target.active,
    }));
  }

  async getTarget(uuid: string): Promise<RedTeamTargetDetail> {
    const response = await this.client.targets.get(uuid);
    return normalizeTargetDetail(response as unknown as Record<string, unknown>);
  }

  async createTarget(
    request: RedTeamTargetCreateRequest,
    opts?: TargetOperationOptions,
  ): Promise<RedTeamTargetDetail> {
    const response = await this.client.targets.create(request as never, opts);
    return normalizeTargetDetail(response as unknown as Record<string, unknown>);
  }

  async updateTarget(
    uuid: string,
    request: RedTeamTargetUpdateRequest,
    opts?: TargetOperationOptions,
  ): Promise<RedTeamTargetDetail> {
    const response = await this.client.targets.update(uuid, request as never, opts);
    return normalizeTargetDetail(response as unknown as Record<string, unknown>);
  }

  async deleteTarget(uuid: string): Promise<void> {
    await this.client.targets.delete(uuid);
  }

  async probeTarget(request: Record<string, unknown>): Promise<Record<string, unknown>> {
    const response = await this.client.targets.probe(request as never);
    const raw = response as unknown as Record<string, unknown>;
    const meta = raw.target_metadata;
    if (meta && typeof meta === 'object') {
      raw.target_metadata = sanitizeTargetMetadata(meta as Record<string, unknown>);
    }
    return raw;
  }

  async getTargetProfile(uuid: string): Promise<Record<string, unknown>> {
    const response = await this.client.targets.getProfile(uuid);
    return response as unknown as Record<string, unknown>;
  }

  async updateTargetProfile(
    uuid: string,
    request: Record<string, unknown>,
  ): Promise<Record<string, unknown>> {
    const response = await this.client.targets.updateProfile(uuid, request as never);
    return response as unknown as Record<string, unknown>;
  }

  async createScan(request: {
    name: string;
    targetUuid: string;
    jobType: string;
    categories?: Record<string, unknown>;
    customPromptSets?: string[];
    attackGoals?: string[];
    streamDepth?: number;
    streamBreadth?: number;
  }): Promise<RedTeamJob> {
    let jobMetadata: Record<string, unknown> = {};
    if (request.jobType === 'STATIC') {
      if (!request.categories) {
        throw new Error(
          'STATIC scans require categories. Pass categories explicitly or use the CLI default.',
        );
      }
      jobMetadata = { categories: request.categories };
    } else if (request.jobType === 'CUSTOM' && request.customPromptSets) {
      jobMetadata = {
        custom_prompt_sets: request.customPromptSets,
      };
    } else if (request.jobType === 'DYNAMIC') {
      jobMetadata = {
        stream_breadth: request.streamBreadth ?? DEFAULT_DYNAMIC_BREADTH,
        stream_depth: request.streamDepth ?? DEFAULT_DYNAMIC_DEPTH,
        ...(request.attackGoals?.length ? { attack_goals: request.attackGoals } : {}),
      };
    }

    const response = await this.client.scans.create({
      name: request.name,
      target: { uuid: request.targetUuid },
      job_type: request.jobType,
      job_metadata: jobMetadata,
    });
    return normalizeJob(response as unknown as Record<string, unknown>);
  }

  async getScan(jobId: string): Promise<RedTeamJob> {
    const response = await this.client.scans.get(jobId);
    return normalizeJob(response as unknown as Record<string, unknown>);
  }

  async listScans(opts?: {
    status?: string;
    jobType?: string;
    targetId?: string;
    limit?: number;
    offset?: number;
  }): Promise<RedTeamJob[]> {
    const sdkOpts: Record<string, unknown> = {};
    if (opts?.status) sdkOpts.status = opts.status;
    if (opts?.jobType) sdkOpts.job_type = opts.jobType;
    if (opts?.targetId) sdkOpts.target_id = opts.targetId;
    if (opts?.limit) sdkOpts.limit = opts.limit;
    if (opts?.offset !== undefined) sdkOpts.skip = opts.offset;

    const response = await this.client.scans.list(sdkOpts);
    return ((response as Record<string, unknown>).data as Record<string, unknown>[]).map(
      normalizeJob,
    );
  }

  async listAllScans(opts?: {
    status?: string;
    jobType?: string;
    targetId?: string;
    limit?: number;
    max?: number;
  }): Promise<RedTeamJob[]> {
    const sdkOpts: Record<string, unknown> = {};
    if (opts?.status) sdkOpts.status = opts.status;
    if (opts?.jobType) sdkOpts.job_type = opts.jobType;
    if (opts?.targetId) sdkOpts.target_id = opts.targetId;
    if (opts?.limit) sdkOpts.limit = opts.limit;
    if (opts?.max !== undefined) sdkOpts.max = opts.max;
    const rows = await this.client.scans.listAll(sdkOpts);
    return rows.map((row) => normalizeJob(row as unknown as Record<string, unknown>));
  }

  async abortScan(jobId: string): Promise<void> {
    await this.client.scans.abort(jobId);
  }

  async getStaticReport(jobId: string): Promise<RedTeamStaticReport> {
    const raw = (await this.client.reports.getStaticReport(jobId)) as Record<string, unknown>;
    const severityReport = raw.severity_report as Record<string, unknown>;
    const stats = (severityReport?.stats ?? []) as Array<Record<string, unknown>>;

    const securityReport = raw.security_report as Record<string, unknown> | undefined;
    const subCategories = (securityReport?.sub_categories ?? []) as Array<Record<string, unknown>>;

    return {
      score: raw.score as number | null | undefined,
      asr: raw.asr as number | null | undefined,
      severityBreakdown: stats.map((s) => ({
        severity: s.severity as string,
        successful: (s.successful ?? 0) as number,
        failed: (s.failed ?? 0) as number,
      })),
      reportSummary: interpolateReportSummary(raw.report_summary as string | null | undefined),
      categories: subCategories.map((sc) => {
        const successful = (sc.successful ?? 0) as number;
        const failed = (sc.failed ?? 0) as number;
        const total = (sc.total ?? successful + failed) as number;
        return {
          id: sc.id as string,
          displayName: sc.display_name as string,
          asr: total > 0 ? (successful / total) * 100 : 0,
          successful,
          failed,
          total,
        };
      }),
    };
  }

  async getDynamicReport(jobId: string): Promise<RedTeamDynamicReport> {
    const raw = (await this.client.reports.getDynamicReport(jobId)) as Record<string, unknown>;
    return {
      score: raw.score as number | undefined,
      asr: raw.asr as number | undefined,
      totalGoals: raw.total_goals as number | undefined,
      goalsAchieved: raw.goals_achieved as number | undefined,
      totalStreams: raw.total_streams as number | undefined,
      totalThreats: raw.total_threats as number | undefined,
      reportSummary: interpolateReportSummary(raw.report_summary as string | null | undefined),
    };
  }

  async getCustomReport(jobId: string): Promise<RedTeamCustomReport> {
    const raw = (await this.client.customAttackReports.getReport(jobId)) as Record<string, unknown>;
    const reports = (raw.custom_attack_reports ?? []) as Array<Record<string, unknown>>;

    return {
      totalPrompts: raw.total_prompts as number,
      totalAttacks: raw.total_attacks as number,
      totalThreats: raw.total_threats as number,
      failedAttacks: raw.failed_attacks as number,
      score: raw.score as number,
      asr: raw.asr as number,
      promptSets: reports.map((r) => ({
        promptSetId: r.prompt_set_id as string,
        promptSetName: r.prompt_set_name as string,
        totalPrompts: r.total_prompts as number,
        totalAttacks: r.total_attacks as number,
        totalThreats: r.total_threats as number,
        threatRate: r.threat_rate as number,
      })),
    };
  }

  async listAttacks(
    jobId: string,
    opts?: { severity?: string; limit?: number },
  ): Promise<{ attacks: RedTeamAttack[]; totalItems?: number }> {
    const response = (await this.client.reports.listAttacks(jobId, opts)) as Record<
      string,
      unknown
    >;
    const data = (response.data ?? []) as Array<Record<string, unknown>>;
    const pagination = response.pagination as { total_items?: number } | undefined;
    return {
      attacks: data.map((a) => ({
        id: a.uuid as string,
        name: a.attack_name as string,
        severity: a.severity as string | undefined,
        category: a.category as string | undefined,
        subCategory: a.sub_category as string | undefined,
        subCategoryDisplayName: a.sub_category_display_name as string | undefined,
        successful: (a.threat ?? false) as boolean,
      })),
      totalItems: pagination?.total_items,
    };
  }

  async listCustomAttacks(
    jobId: string,
    opts?: { limit?: number },
  ): Promise<RedTeamCustomAttack[]> {
    const response = await this.client.customAttackReports.listCustomAttacks(jobId, opts);
    return ((response as Record<string, unknown>).data as Array<Record<string, unknown>>).map(
      (a) => ({
        promptId: a.prompt_id as string,
        promptText: a.prompt_text as string,
        goal: a.goal as string | undefined,
        threat: (a.threat ?? false) as boolean,
        asr: a.asr as number | undefined,
        promptSetName: a.prompt_set_name as string | undefined,
      }),
    );
  }

  async getCategories(): Promise<RedTeamCategory[]> {
    const response = (await this.client.scans.getCategories()) as Array<Record<string, unknown>>;
    return response.map((c) => ({
      id: c.id as string,
      displayName: c.display_name as string,
      description: c.description as string | undefined,
      subCategories: ((c.sub_categories ?? []) as Array<Record<string, unknown>>).map((sc) => ({
        id: sc.id as string,
        displayName: sc.display_name as string,
        description: sc.description as string | undefined,
      })),
    }));
  }

  async waitForCompletion(
    jobId: string,
    onProgress?: (job: RedTeamJob) => void,
    intervalMs = 5000,
  ): Promise<RedTeamJob> {
    const delay = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

    // eslint-disable-next-line no-constant-condition
    while (true) {
      const job = await this.getScan(jobId);
      onProgress?.(job);

      if (job.status === 'FAILED') {
        throw new Error(`Scan ${jobId} failed`);
      }
      if (TERMINAL_STATUSES.has(job.status)) {
        return job;
      }
      await delay(intervalMs);
    }
  }

  async listChannels(
    opts?: RedTeamChannelListOptions,
  ): Promise<{ channels: RedTeamChannel[]; totalItems?: number }> {
    const sdkOpts: Record<string, unknown> = {};
    if (opts?.limit != null) sdkOpts.limit = opts.limit;
    if (opts?.offset != null) sdkOpts.skip = opts.offset;
    if (opts?.search) sdkOpts.search = opts.search;
    if (opts?.status) sdkOpts.status = opts.status;

    const raw = (await this.client.networkBroker.listChannels(sdkOpts)) as Record<string, unknown>;
    const pagination = raw.pagination as Record<string, unknown> | undefined;
    return {
      channels: ((raw.data ?? []) as Array<Record<string, unknown>>).map(normalizeChannel),
      totalItems: pagination?.total_items as number | undefined,
    };
  }

  async listAllChannels(
    opts: RedTeamChannelListOptions & { max?: number } = {},
  ): Promise<RedTeamChannel[]> {
    const limit = opts.limit ?? 100;
    const cap = opts.max === 0 ? Number.POSITIVE_INFINITY : (opts.max ?? 10_000);
    const rows: RedTeamChannel[] = [];
    for (let offset = 0; rows.length < cap; offset += limit) {
      const page = await this.listChannels({ ...opts, limit, offset });
      rows.push(...page.channels.slice(0, cap - rows.length));
      if (
        page.channels.length < limit ||
        rows.length >= (page.totalItems ?? Number.POSITIVE_INFINITY)
      )
        break;
    }
    return rows;
  }

  async getChannel(channelId: string): Promise<RedTeamChannel> {
    const raw = (await this.client.networkBroker.getChannel(channelId)) as Record<string, unknown>;
    return normalizeChannel(raw);
  }

  async createChannel(request: RedTeamChannelCreateRequest): Promise<RedTeamChannel> {
    const body: Record<string, unknown> = { name: request.name };
    if (request.description !== undefined) body.description = request.description;
    const raw = (await this.client.networkBroker.createChannel(
      body as unknown as Parameters<RedTeamClient['networkBroker']['createChannel']>[0],
    )) as Record<string, unknown>;
    return normalizeChannel(raw);
  }

  async updateChannel(
    channelId: string,
    request: RedTeamChannelUpdateRequest,
  ): Promise<RedTeamChannel> {
    const body: Record<string, unknown> = {};
    if (request.name !== undefined) body.name = request.name;
    if (request.description !== undefined) body.description = request.description;
    const raw = (await this.client.networkBroker.updateChannel(
      channelId,
      body as unknown as Parameters<RedTeamClient['networkBroker']['updateChannel']>[1],
    )) as Record<string, unknown>;
    return normalizeChannel(raw);
  }

  async getChannelStats(): Promise<RedTeamChannelStats> {
    const raw = (await this.client.networkBroker.getChannelStats()) as Record<string, unknown>;
    return {
      serverDomain: raw.network_channels_server_domain as string | null | undefined,
      dockerRegistry: raw.docker_registry as string | null | undefined,
      helmChart: raw.helm_chart as string | null | undefined,
      dockerImage: raw.docker_image as string | null | undefined,
      onlineChannels: raw.online_channels as number | null | undefined,
      totalChannels: raw.total_channels as number | null | undefined,
      clientVersion: raw.client_version as string | null | undefined,
    };
  }

  async getLanguages(management = false): Promise<RedTeamLanguages> {
    const raw = (await (management
      ? this.client.getManagementLanguages()
      : this.client.getLanguages())) as Record<string, unknown>;
    return {
      multilingualEnabled: Boolean(raw.multilingual_enabled),
      supportedJobTypes: (raw.supported_job_types ?? []) as string[],
      languages: ((raw.languages ?? []) as Array<Record<string, unknown>>).map((l) => ({
        code: l.code as string,
        name: l.name as string,
      })),
    };
  }

  async getTargetProfileErrorLogs(
    targetId: string,
    opts?: { limit?: number; offset?: number; search?: string },
  ): Promise<{ logs: RedTeamErrorLog[]; totalItems?: number }> {
    const sdkOpts: Record<string, unknown> = {};
    if (opts?.limit != null) sdkOpts.limit = opts.limit;
    if (opts?.offset != null) sdkOpts.skip = opts.offset;
    if (opts?.search) sdkOpts.search = opts.search;

    const raw = (await this.client.getTargetProfileErrorLogs(targetId, sdkOpts)) as Record<
      string,
      unknown
    >;
    const pagination = raw.pagination as Record<string, unknown> | undefined;
    return {
      logs: ((raw.data ?? []) as Array<Record<string, unknown>>).map(normalizeErrorLog),
      totalItems: pagination?.total_items as number | undefined,
    };
  }

  // -------------------------------------------------------------------------
  // Custom target adapters (SDK 0.16.0)
  // -------------------------------------------------------------------------

  async listAdapters(
    opts?: RedTeamAdapterListOptions,
  ): Promise<{ adapters: RedTeamAdapterListItem[]; totalItems?: number }> {
    const sdkOpts: Record<string, unknown> = {};
    if (opts?.limit != null) sdkOpts.limit = opts.limit;
    if (opts?.offset != null) sdkOpts.skip = opts.offset;
    if (opts?.search) sdkOpts.search = opts.search;

    const raw = (await this.client.adapters.list(sdkOpts)) as Record<string, unknown>;
    const pagination = raw.pagination as Record<string, unknown> | undefined;
    return {
      adapters: ((raw.data ?? []) as Array<Record<string, unknown>>).map(normalizeAdapterListItem),
      totalItems: pagination?.total_items as number | undefined,
    };
  }

  async listAllAdapters(
    opts: RedTeamAdapterListOptions & { max?: number } = {},
  ): Promise<RedTeamAdapterListItem[]> {
    const sdkOpts: Record<string, unknown> = {};
    if (opts.limit != null) sdkOpts.limit = opts.limit;
    if (opts.search) sdkOpts.search = opts.search;
    if (opts.max !== undefined) sdkOpts.max = opts.max;
    const rows = await this.client.adapters.listAll(sdkOpts);
    return rows.map((row) => normalizeAdapterListItem(row as unknown as Record<string, unknown>));
  }

  async getAdapter(uuid: string): Promise<RedTeamAdapterDetail> {
    const raw = (await this.client.adapters.get(uuid)) as Record<string, unknown>;
    return normalizeAdapterDetail(raw);
  }

  async createAdapter(
    request: RedTeamAdapterCreateRequest,
    validate?: boolean,
  ): Promise<RedTeamAdapterDetail> {
    const body: Record<string, unknown> = {
      name: request.name,
      script_b64: request.scriptB64,
      prompt: request.prompt,
    };
    if (request.description !== undefined) body.description = request.description;
    if (request.networkBrokerChannelUuid !== undefined) {
      body.network_broker_channel_uuid = request.networkBrokerChannelUuid;
    }
    if (request.variables !== undefined) body.variables = toWireVariables(request.variables);

    const raw = (await this.client.adapters.create(
      // biome-ignore lint/suspicious/noExplicitAny: body is assembled dynamically; SDK validates the shape
      body as any,
      validate === undefined ? undefined : { validate },
    )) as Record<string, unknown>;
    return normalizeAdapterDetail(raw);
  }

  async updateAdapter(
    uuid: string,
    overrides: RedTeamAdapterUpdateOverrides,
    validate?: boolean,
  ): Promise<RedTeamAdapterDetail> {
    // Upstream update is a full-replacement PUT and omitting a variable key
    // DELETES it — read-modify-write so a partial CLI update cannot silently
    // wipe state. `prompt` is never stored, so the caller must supply it.
    const current = await this.getAdapter(uuid);
    const body: Record<string, unknown> = {
      name: overrides.name ?? current.name,
      script_b64: overrides.scriptB64 ?? current.scriptB64,
      prompt: overrides.prompt,
      variables: overrides.variables
        ? toWireVariables(overrides.variables)
        : preserveVariablesForUpdate(current.variables),
    };
    const description = overrides.description ?? current.description;
    if (description != null) body.description = description;
    const channel = overrides.networkBrokerChannelUuid ?? current.networkBrokerChannelUuid;
    if (channel != null) body.network_broker_channel_uuid = channel;

    const raw = (await this.client.adapters.update(
      uuid,
      // biome-ignore lint/suspicious/noExplicitAny: body is assembled dynamically; SDK validates the shape
      body as any,
      validate === undefined ? undefined : { validate },
    )) as Record<string, unknown>;
    return normalizeAdapterDetail(raw);
  }

  async deleteAdapter(uuid: string): Promise<void> {
    await this.client.adapters.delete(uuid);
  }

  async validateAdapter(
    request: RedTeamAdapterValidateRequest,
  ): Promise<RedTeamAdapterValidationResult> {
    let variables = request.variables ? toWireVariables(request.variables) : undefined;
    if (variables === undefined && request.adapterUuid) {
      // validate() requires the FULL variables array — adapter_uuid only
      // resolves redacted values within what is sent; it does not supply the
      // list. Omitting it fails upstream with a bare KeyError.
      const adapter = await this.getAdapter(request.adapterUuid);
      variables = preserveVariablesForUpdate(adapter.variables);
    }
    const body: Record<string, unknown> = {
      script_b64: request.scriptB64,
      network_broker_channel_uuid: request.networkBrokerChannelUuid,
      prompt: request.prompt,
    };
    if (variables !== undefined) body.variables = variables;
    if (request.adapterUuid !== undefined) body.adapter_uuid = request.adapterUuid;

    // biome-ignore lint/suspicious/noExplicitAny: body is assembled dynamically; SDK validates the shape
    const raw = (await this.client.adapters.validate(body as any)) as Record<string, unknown>;
    return {
      validated: raw.validated as boolean,
      stdout: raw.stdout as string | null | undefined,
      stderr: raw.stderr as string | null | undefined,
      traceback: raw.traceback as string | null | undefined,
    };
  }
}
