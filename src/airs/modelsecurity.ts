import { ModelSecurityClient, type ModelSecurityClientOptions } from '@cdot65/prisma-airs-sdk';
import type {
  ModelSecurityEvaluation,
  ModelSecurityFile,
  ModelSecurityFileListOptions,
  ModelSecurityGroup,
  ModelSecurityGroupCreateRequest,
  ModelSecurityGroupListOptions,
  ModelSecurityGroupUpdateRequest,
  ModelSecurityLabel,
  ModelSecurityPyPIAuth,
  ModelSecurityRule,
  ModelSecurityRuleEditableField,
  ModelSecurityRuleInstance,
  ModelSecurityRuleInstanceListOptions,
  ModelSecurityRuleInstanceUpdateRequest,
  ModelSecurityRuleListOptions,
  ModelSecurityScan,
  ModelSecurityScanListOptions,
  ModelSecurityService,
  ModelSecurityViolation,
} from './types.js';

/** Normalize an SDK security group response. */
function normalizeGroup(raw: Record<string, unknown>): ModelSecurityGroup {
  return {
    uuid: raw.uuid as string,
    name: raw.name as string,
    description: raw.description as string,
    sourceType: raw.source_type as string,
    state: raw.state as string,
    createdAt: raw.created_at as string,
    updatedAt: raw.updated_at as string,
  };
}

/** Normalize an SDK rule instance response. */
function normalizeRuleInstance(raw: Record<string, unknown>): ModelSecurityRuleInstance {
  return {
    uuid: raw.uuid as string,
    securityGroupUuid: raw.security_group_uuid as string,
    securityRuleUuid: raw.security_rule_uuid as string,
    state: raw.state as string,
    createdAt: raw.created_at as string,
    updatedAt: raw.updated_at as string,
    rule: raw.rule as Record<string, unknown>,
    fieldValues: (raw.field_values ?? {}) as Record<string, unknown>,
  };
}

/** Normalize an SDK security rule response. */
function normalizeRule(raw: Record<string, unknown>): ModelSecurityRule {
  const editableFields = (raw.editable_fields ?? []) as Array<Record<string, unknown>>;
  return {
    uuid: raw.uuid as string,
    name: raw.name as string,
    description: raw.description as string,
    ruleType: raw.rule_type as string,
    compatibleSources: raw.compatible_sources as string[],
    defaultState: raw.default_state as string,
    remediation: raw.remediation as ModelSecurityRule['remediation'],
    editableFields: editableFields.map(
      (f) =>
        ({
          attributeName: f.attribute_name as string,
          type: f.type as string,
          displayName: f.display_name as string,
          displayType: f.display_type as string,
          description: f.description as string | undefined,
          dropdownValues: f.dropdown_values as Array<{ value: string; label: string }> | undefined,
        }) as ModelSecurityRuleEditableField,
    ),
    constantValues: (raw.constant_values ?? {}) as Record<string, unknown>,
    defaultValues: (raw.default_values ?? {}) as Record<string, unknown>,
  };
}

/** Normalize an SDK scan response. */
function normalizeScan(raw: Record<string, unknown>): ModelSecurityScan {
  const summary = raw.eval_summary as Record<string, unknown> | null;
  return {
    uuid: raw.uuid as string,
    evalOutcome: (raw.eval_outcome ?? '') as string,
    modelUri: (raw.model_uri ?? '') as string,
    scanOrigin: (raw.scan_origin ?? '') as string,
    sourceType: (raw.source_type ?? '') as string,
    securityGroupName: (raw.security_group_name ?? '') as string,
    evalSummary: summary
      ? {
          rulesFailed: (summary.rules_failed ?? 0) as number,
          rulesPassed: (summary.rules_passed ?? 0) as number,
          totalRules: (summary.total_rules ?? 0) as number,
        }
      : null,
    createdAt: raw.created_at as string,
    updatedAt: raw.updated_at as string,
    labels: (raw.labels ?? []) as Array<{ key: string; value: string }>,
  };
}

/** Normalize an SDK evaluation response. */
function normalizeEvaluation(raw: Record<string, unknown>): ModelSecurityEvaluation {
  return {
    uuid: raw.uuid as string,
    result: raw.result as string,
    violationCount: (raw.violation_count ?? 0) as number,
    ruleInstanceUuid: (raw.rule_instance_uuid ?? '') as string,
    ruleName: raw.rule_name as string,
    ruleDescription: (raw.rule_description ?? '') as string,
    ruleInstanceState: (raw.rule_instance_state ?? '') as string,
  };
}

/** Normalize an SDK violation response. */
function normalizeViolation(raw: Record<string, unknown>): ModelSecurityViolation {
  return {
    uuid: raw.uuid as string,
    description: raw.description as string,
    threat: (raw.threat ?? '') as string,
    threatDescription: (raw.threat_description ?? '') as string,
    file: (raw.file ?? '') as string,
    ruleName: raw.rule_name as string,
    ruleDescription: (raw.rule_description ?? '') as string,
    ruleInstanceState: (raw.rule_instance_state ?? '') as string,
  };
}

/** Normalize an SDK file response. */
function normalizeFile(raw: Record<string, unknown>): ModelSecurityFile {
  return {
    uuid: raw.uuid as string,
    path: (raw.path ?? '') as string,
    type: raw.type as string,
    formats: (raw.formats ?? []) as string[],
    result: raw.result as string,
  };
}

/**
 * Wraps the SDK's ModelSecurityClient to implement ModelSecurityService.
 * Provides security group CRUD, rule browsing, scan operations, and label management.
 */
export class SdkModelSecurityService implements ModelSecurityService {
  private client: ModelSecurityClient;

  constructor(opts?: ModelSecurityClientOptions) {
    this.client = new ModelSecurityClient(opts);
  }

  // -----------------------------------------------------------------------
  // Security Groups
  // -----------------------------------------------------------------------

  async listGroups(
    opts?: ModelSecurityGroupListOptions,
  ): Promise<{ totalItems: number; groups: ModelSecurityGroup[] }> {
    const sdkOpts: Record<string, unknown> = {};
    if (opts?.sourceTypes) sdkOpts.source_types = opts.sourceTypes;
    if (opts?.searchQuery) sdkOpts.search_query = opts.searchQuery;
    if (opts?.sortField) sdkOpts.sort_field = opts.sortField;
    if (opts?.sortDir) sdkOpts.sort_dir = opts.sortDir;
    if (opts?.enabledRules) sdkOpts.enabled_rules = opts.enabledRules;
    if (opts?.skip !== undefined) sdkOpts.skip = opts.skip;
    if (opts?.limit !== undefined) sdkOpts.limit = opts.limit;

    const response = await this.client.securityGroups.list(sdkOpts as never);
    const raw = response as unknown as {
      pagination: { total_items?: number };
      security_groups: Array<Record<string, unknown>>;
    };
    return {
      totalItems: raw.pagination.total_items ?? 0,
      groups: raw.security_groups.map(normalizeGroup),
    };
  }

  async getGroup(uuid: string): Promise<ModelSecurityGroup> {
    const response = await this.client.securityGroups.get(uuid);
    return normalizeGroup(response as unknown as Record<string, unknown>);
  }

  async createGroup(request: ModelSecurityGroupCreateRequest): Promise<ModelSecurityGroup> {
    const sdkRequest: Record<string, unknown> = {
      name: request.name,
      source_type: request.sourceType,
    };
    if (request.description !== undefined) sdkRequest.description = request.description;
    if (request.ruleConfigurations) sdkRequest.rule_configurations = request.ruleConfigurations;

    const response = await this.client.securityGroups.create(sdkRequest as never);
    return normalizeGroup(response as unknown as Record<string, unknown>);
  }

  async updateGroup(
    uuid: string,
    request: ModelSecurityGroupUpdateRequest,
  ): Promise<ModelSecurityGroup> {
    const response = await this.client.securityGroups.update(uuid, request as never);
    return normalizeGroup(response as unknown as Record<string, unknown>);
  }

  async deleteGroup(uuid: string): Promise<void> {
    await this.client.securityGroups.delete(uuid);
  }

  // -----------------------------------------------------------------------
  // Rule Instances
  // -----------------------------------------------------------------------

  async listRuleInstances(
    groupUuid: string,
    opts?: ModelSecurityRuleInstanceListOptions,
  ): Promise<{ totalItems: number; ruleInstances: ModelSecurityRuleInstance[] }> {
    const sdkOpts: Record<string, unknown> = {};
    if (opts?.securityRuleUuid) sdkOpts.security_rule_uuid = opts.securityRuleUuid;
    if (opts?.state) sdkOpts.state = opts.state;
    if (opts?.skip !== undefined) sdkOpts.skip = opts.skip;
    if (opts?.limit !== undefined) sdkOpts.limit = opts.limit;

    const response = await this.client.securityGroups.listRuleInstances(
      groupUuid,
      sdkOpts as never,
    );
    const raw = response as unknown as {
      pagination: { total_items?: number };
      rule_instances: Array<Record<string, unknown>>;
    };
    return {
      totalItems: raw.pagination.total_items ?? 0,
      ruleInstances: raw.rule_instances.map(normalizeRuleInstance),
    };
  }

  async getRuleInstance(
    groupUuid: string,
    instanceUuid: string,
  ): Promise<ModelSecurityRuleInstance> {
    const response = await this.client.securityGroups.getRuleInstance(groupUuid, instanceUuid);
    return normalizeRuleInstance(response as unknown as Record<string, unknown>);
  }

  async updateRuleInstance(
    groupUuid: string,
    instanceUuid: string,
    request: ModelSecurityRuleInstanceUpdateRequest,
  ): Promise<ModelSecurityRuleInstance> {
    const sdkRequest: Record<string, unknown> = {
      security_group_uuid: groupUuid,
    };
    if (request.state !== undefined) sdkRequest.state = request.state;
    if (request.fieldValues !== undefined) sdkRequest.field_values = request.fieldValues;

    const response = await this.client.securityGroups.updateRuleInstance(
      groupUuid,
      instanceUuid,
      sdkRequest as never,
    );
    return normalizeRuleInstance(response as unknown as Record<string, unknown>);
  }

  // -----------------------------------------------------------------------
  // Security Rules
  // -----------------------------------------------------------------------

  async listRules(
    opts?: ModelSecurityRuleListOptions,
  ): Promise<{ totalItems: number; rules: ModelSecurityRule[] }> {
    const sdkOpts: Record<string, unknown> = {};
    if (opts?.sourceType) sdkOpts.source_type = opts.sourceType;
    if (opts?.searchQuery) sdkOpts.search_query = opts.searchQuery;
    if (opts?.skip !== undefined) sdkOpts.skip = opts.skip;
    if (opts?.limit !== undefined) sdkOpts.limit = opts.limit;

    const response = await this.client.securityRules.list(sdkOpts as never);
    const raw = response as unknown as {
      pagination: { total_items?: number };
      rules: Array<Record<string, unknown>>;
    };
    return {
      totalItems: raw.pagination.total_items ?? 0,
      rules: raw.rules.map(normalizeRule),
    };
  }

  async getRule(uuid: string): Promise<ModelSecurityRule> {
    const response = await this.client.securityRules.get(uuid);
    return normalizeRule(response as unknown as Record<string, unknown>);
  }

  // -----------------------------------------------------------------------
  // Scans
  // -----------------------------------------------------------------------

  async createScan(request: Record<string, unknown>): Promise<ModelSecurityScan> {
    const response = await this.client.scans.create(request as never);
    return normalizeScan(response as unknown as Record<string, unknown>);
  }

  async listScans(
    opts?: ModelSecurityScanListOptions,
  ): Promise<{ totalItems: number; scans: ModelSecurityScan[] }> {
    const sdkOpts: Record<string, unknown> = {};
    if (opts?.evalOutcome) sdkOpts.eval_outcome = opts.evalOutcome;
    if (opts?.sourceType) sdkOpts.source_type = opts.sourceType;
    if (opts?.scanOrigin) sdkOpts.scan_origin = opts.scanOrigin;
    if (opts?.search) sdkOpts.search = opts.search;
    if (opts?.skip !== undefined) sdkOpts.skip = opts.skip;
    if (opts?.limit !== undefined) sdkOpts.limit = opts.limit;

    const response = await this.client.scans.list(sdkOpts as never);
    const raw = response as unknown as {
      pagination: { total_items?: number };
      scans: Array<Record<string, unknown>>;
    };
    return {
      totalItems: raw.pagination.total_items ?? 0,
      scans: raw.scans.map(normalizeScan),
    };
  }

  async getScan(uuid: string): Promise<ModelSecurityScan> {
    const response = await this.client.scans.get(uuid);
    return normalizeScan(response as unknown as Record<string, unknown>);
  }

  // -----------------------------------------------------------------------
  // Evaluations
  // -----------------------------------------------------------------------

  async getEvaluations(
    scanUuid: string,
    opts?: { skip?: number; limit?: number },
  ): Promise<{ totalItems: number; evaluations: ModelSecurityEvaluation[] }> {
    const response = await this.client.scans.getEvaluations(scanUuid, opts as never);
    const raw = response as unknown as {
      pagination: { total_items?: number };
      evaluations: Array<Record<string, unknown>>;
    };
    return {
      totalItems: raw.pagination.total_items ?? 0,
      evaluations: raw.evaluations.map(normalizeEvaluation),
    };
  }

  async getEvaluation(uuid: string): Promise<ModelSecurityEvaluation> {
    const response = await this.client.scans.getEvaluation(uuid);
    return normalizeEvaluation(response as unknown as Record<string, unknown>);
  }

  // -----------------------------------------------------------------------
  // Violations
  // -----------------------------------------------------------------------

  async getViolations(
    scanUuid: string,
    opts?: { skip?: number; limit?: number },
  ): Promise<{ totalItems: number; violations: ModelSecurityViolation[] }> {
    const response = await this.client.scans.getViolations(scanUuid, opts as never);
    const raw = response as unknown as {
      pagination: { total_items?: number };
      violations: Array<Record<string, unknown>>;
    };
    return {
      totalItems: raw.pagination.total_items ?? 0,
      violations: raw.violations.map(normalizeViolation),
    };
  }

  async getViolation(uuid: string): Promise<ModelSecurityViolation> {
    const response = await this.client.scans.getViolation(uuid);
    return normalizeViolation(response as unknown as Record<string, unknown>);
  }

  // -----------------------------------------------------------------------
  // Files
  // -----------------------------------------------------------------------

  async getFiles(
    scanUuid: string,
    opts?: ModelSecurityFileListOptions,
  ): Promise<{ totalItems: number; files: ModelSecurityFile[] }> {
    const response = await this.client.scans.getFiles(scanUuid, opts as never);
    const raw = response as unknown as {
      pagination: { total_items?: number };
      files: Array<Record<string, unknown>>;
    };
    return {
      totalItems: raw.pagination.total_items ?? 0,
      files: raw.files.map(normalizeFile),
    };
  }

  // -----------------------------------------------------------------------
  // Labels
  // -----------------------------------------------------------------------

  async addLabels(scanUuid: string, labels: ModelSecurityLabel[]): Promise<void> {
    await this.client.scans.addLabels(scanUuid, { labels } as never);
  }

  async setLabels(scanUuid: string, labels: ModelSecurityLabel[]): Promise<void> {
    await this.client.scans.setLabels(scanUuid, { labels } as never);
  }

  async deleteLabels(scanUuid: string, keys: string[]): Promise<void> {
    await this.client.scans.deleteLabels(scanUuid, keys);
  }

  async getLabelKeys(opts?: {
    skip?: number;
    limit?: number;
  }): Promise<{ totalItems: number; keys: string[] }> {
    const response = await this.client.scans.getLabelKeys(opts as never);
    const raw = response as unknown as {
      pagination: { total_items?: number };
      keys: string[];
    };
    return {
      totalItems: raw.pagination.total_items ?? 0,
      keys: raw.keys,
    };
  }

  async getLabelValues(
    key: string,
    opts?: { skip?: number; limit?: number },
  ): Promise<{ totalItems: number; values: string[] }> {
    const response = await this.client.scans.getLabelValues(key, opts as never);
    const raw = response as unknown as {
      pagination: { total_items?: number };
      values: string[];
    };
    return {
      totalItems: raw.pagination.total_items ?? 0,
      values: raw.values,
    };
  }

  // -----------------------------------------------------------------------
  // PyPI Auth
  // -----------------------------------------------------------------------

  async getPyPIAuth(): Promise<ModelSecurityPyPIAuth> {
    const response = await this.client.getPyPIAuth();
    const raw = response as unknown as Record<string, unknown>;
    return {
      url: raw.url as string,
      expiresAt: raw.expires_at as string,
    };
  }
}
