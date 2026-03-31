import { RedTeamClient, type RedTeamClientOptions } from '@cdot65/prisma-airs-sdk';
import type {
  EulaContent,
  EulaStatus,
  InstanceDetail,
  InstanceRequest,
  InstanceResponse,
  RedTeamAttack,
  RedTeamCategory,
  RedTeamCustomAttack,
  RedTeamCustomReport,
  RedTeamJob,
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

/** Normalize an SDK target response into a RedTeamTargetDetail. */
function normalizeTargetDetail(raw: Record<string, unknown>): RedTeamTargetDetail {
  return {
    uuid: raw.uuid as string,
    name: raw.name as string,
    status: raw.status as string,
    targetType: raw.target_type as string | undefined,
    active: raw.active as boolean,
    connectionParams: raw.connection_params as Record<string, unknown> | undefined,
    background: raw.background as RedTeamTargetDetail['background'],
    additionalContext: raw.additional_context as RedTeamTargetDetail['additionalContext'],
    metadata: raw.metadata as RedTeamTargetDetail['metadata'],
  };
}

/**
 * Wraps the SDK's RedTeamClient to implement RedTeamService.
 * Provides scan creation, status polling, report retrieval, and target/category listing.
 */
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
    const response = await this.client.targets.list();
    return ((response as Record<string, unknown>).data as Record<string, unknown>[]).map((t) => ({
      uuid: t.uuid as string,
      name: t.name as string,
      status: t.status as string,
      targetType: t.target_type as string | undefined,
      active: t.active as boolean,
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
    return response as unknown as Record<string, unknown>;
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
  }): Promise<RedTeamJob> {
    let jobMetadata: Record<string, unknown> = {};
    if (request.jobType === 'STATIC' && request.categories) {
      jobMetadata = { categories: request.categories };
    } else if (request.jobType === 'CUSTOM' && request.customPromptSets) {
      jobMetadata = {
        custom_prompt_sets: request.customPromptSets,
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
  }): Promise<RedTeamJob[]> {
    const sdkOpts: Record<string, unknown> = {};
    if (opts?.status) sdkOpts.status = opts.status;
    if (opts?.jobType) sdkOpts.job_type = opts.jobType;
    if (opts?.targetId) sdkOpts.target_id = opts.targetId;
    if (opts?.limit) sdkOpts.limit = opts.limit;

    const response = await this.client.scans.list(sdkOpts);
    return ((response as Record<string, unknown>).data as Record<string, unknown>[]).map(
      normalizeJob,
    );
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
      reportSummary: raw.report_summary as string | null | undefined,
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
  ): Promise<RedTeamAttack[]> {
    const response = await this.client.reports.listAttacks(jobId, opts);
    return ((response as Record<string, unknown>).data as Array<Record<string, unknown>>).map(
      (a) => ({
        id: a.uuid as string,
        name: a.attack_name as string,
        severity: a.severity as string | undefined,
        category: a.category as string | undefined,
        subCategory: a.sub_category as string | undefined,
        successful: a.successful as boolean,
      }),
    );
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
}
