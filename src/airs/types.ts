/**
 * AIRS integration types — scan results and service interfaces for the
 * Prisma AIRS scanner, topic management, and red team APIs.
 */

import type {
  CreateCustomTopicRequest,
  CreateSecurityProfileRequest,
  CustomTopic as SdkCustomTopic,
} from '@cdot65/prisma-airs-sdk';

/** Enriched topic entry read from a profile's policy. */
export interface ProfileTopic {
  topicId: string;
  topicName: string;
  action: 'allow' | 'block';
  description: string;
  examples: string[];
}

// ---------------------------------------------------------------------------
// SDK re-exports — upstream types used across the AIRS layer
// ---------------------------------------------------------------------------
export type { CreateCustomTopicRequest, CreateSecurityProfileRequest, SdkCustomTopic };

// ---------------------------------------------------------------------------
// Scan result — normalized output from a single AIRS prompt scan
// ---------------------------------------------------------------------------
/** Normalized output from a single AIRS prompt scan. */
export interface ScanResult {
  scanId: string;
  reportId: string;
  action: 'allow' | 'block';
  /** Whether the topic guardrail was triggered for this prompt. */
  triggered: boolean;
  category?: string;
  raw?: unknown;
}

// ---------------------------------------------------------------------------
// Runtime scan result — normalized output from sync/async AIRS prompt scans
// ---------------------------------------------------------------------------

/** Normalized result from a runtime prompt scan (sync or async). */
export interface RuntimeScanResult {
  prompt: string;
  response?: string;
  scanId: string;
  reportId: string;
  action: 'allow' | 'block';
  category: string;
  triggered: boolean;
  detections: Record<string, boolean>;
  error?: string;
}

/** Terminal action emitted by the reliable bulk-scan path. */
export type BulkScanAction = RuntimeScanResult['action'] | 'failed';

/** A prompt paired with its stable position and AIRS request ID. */
export interface IndexedPrompt {
  index: number;
  prompt: string;
}

/** Correlation data for one prompt accepted in an async AIRS submission. */
export interface BatchEntry extends IndexedPrompt {
  scanId: string;
  reqId: number;
}

/** Receipt for exactly one SDK async submission (at most twenty prompts). */
export interface SubmittedBatch {
  scanId: string;
  reportId?: string;
  entries: BatchEntry[];
}

/** A normalized async result with its stable input position and AIRS request ID. */
export interface BulkScanResult extends Omit<RuntimeScanResult, 'action'> {
  index: number;
  reqId: number;
  action: BulkScanAction;
}

/** Backwards-compatible contract for the original runtime scanning operations. */
export interface RuntimeService {
  /** Scan a single prompt (and optional response) synchronously. */
  scanPrompt(profileName: string, prompt: string, response?: string): Promise<RuntimeScanResult>;
  /** @deprecated Use ReliableRuntimeService.submitBatch to preserve per-prompt correlation. */
  submitBulkScan(profileName: string, prompts: string[], sessionId?: string): Promise<string[]>;
  /** @deprecated Use ReliableRuntimeService.pollBatch to preserve per-prompt correlation. */
  pollResults(scanIds: string[], intervalMs?: number): Promise<RuntimeScanResult[]>;
}

/** Runtime scanning contract with item-correlated, resumable bulk operations. */
export interface ReliableRuntimeService extends RuntimeService {
  /** Submit one SDK-sized group of indexed prompts for async scanning. */
  submitBatch(
    profileName: string,
    prompts: IndexedPrompt[],
    sessionId?: string,
    retryOpts?: {
      maxRetries?: number;
      baseDelayMs?: number;
      maxNoProgressPolls?: number;
      onRetry?: (attempt: number, delayMs: number) => void;
      onProgress?: (results: BulkScanResult[]) => void | Promise<void>;
    },
  ): Promise<SubmittedBatch>;
  /** Poll one async submission and return one result per prompt, ordered by input index. */
  pollBatch(
    batch: SubmittedBatch,
    intervalMs?: number,
    retryOpts?: {
      maxRetries?: number;
      baseDelayMs?: number;
      maxNoProgressPolls?: number;
      onRetry?: (attempt: number, delayMs: number) => void;
      onProgress?: (results: BulkScanResult[]) => void | Promise<void>;
    },
  ): Promise<BulkScanResult[]>;
}

// ---------------------------------------------------------------------------
// Service interfaces — contracts for scan and topic management adapters
// ---------------------------------------------------------------------------

/** Contract for AIRS prompt scanning operations. */
export interface ScanService {
  /** Scan a single prompt against a security profile. */
  scan(profileName: string, prompt: string, sessionId?: string): Promise<ScanResult>;
  /** Scan multiple prompts with concurrency control. */
  scanBatch(
    profileName: string,
    prompts: string[],
    concurrency?: number,
    sessionId?: string,
  ): Promise<ScanResult[]>;
}

/** Contract for custom prompt set operations in AI Red Team. */
export interface PromptSetService {
  /** Create a new custom prompt set. */
  createPromptSet(name: string, description?: string): Promise<{ uuid: string; name: string }>;
  /** Add a prompt to an existing prompt set. */
  addPrompt(
    promptSetId: string,
    prompt: string,
    goal?: string,
  ): Promise<{ uuid: string; prompt: string }>;
  /** List all custom prompt sets. */
  listPromptSets(): Promise<Array<{ uuid: string; name: string; active: boolean }>>;
  /** Get prompt set details. */
  getPromptSet(uuid: string): Promise<PromptSetDetail>;
  /** Update prompt set name/description. */
  updatePromptSet(
    uuid: string,
    request: { name?: string; description?: string },
  ): Promise<PromptSetDetail>;
  /** Archive or unarchive a prompt set. */
  archivePromptSet(uuid: string, archive: boolean): Promise<void>;
  /** Get prompt set version info with stats. */
  getPromptSetVersionInfo(uuid: string): Promise<PromptSetVersionInfo>;
  /** Download CSV template for a prompt set. */
  downloadTemplate(uuid: string): Promise<string>;
  /** Upload CSV file to a prompt set. */
  uploadPromptsCsv(uuid: string, file: Blob): Promise<{ message: string; status: number }>;
  /** List prompts in a prompt set. */
  listPrompts(setUuid: string, opts?: { limit?: number; skip?: number }): Promise<PromptDetail[]>;
  /** Get a single prompt. */
  getPrompt(setUuid: string, promptUuid: string): Promise<PromptDetail>;
  /** Update a prompt. */
  updatePrompt(
    setUuid: string,
    promptUuid: string,
    request: { prompt?: string; goal?: string },
  ): Promise<PromptDetail>;
  /** Delete a prompt. */
  deletePrompt(setUuid: string, promptUuid: string): Promise<void>;
  /** List property names. */
  getPropertyNames(): Promise<string[]>;
  /** Create a property name. */
  createPropertyName(name: string): Promise<MutationResponse>;
  /** Get values for a property. */
  getPropertyValues(name: string): Promise<PropertyValueList>;
  /** Create a property value. */
  createPropertyValue(name: string, value: string): Promise<MutationResponse>;
}

// ---------------------------------------------------------------------------
// Red Team types — normalized shapes for red team scan operations
// ---------------------------------------------------------------------------

/** Normalized red team job/scan info. */
export interface RedTeamJob {
  uuid: string;
  name: string;
  status: string;
  jobType: string;
  targetId: string;
  targetName?: string;
  score?: number | null;
  asr?: number | null;
  total?: number | null;
  completed?: number | null;
  createdAt?: string | null;
}

/** Normalized red team target info. */
export interface RedTeamTarget {
  uuid: string;
  name: string;
  status: string;
  targetType?: string;
  active: boolean;
}

/** Detailed target info with connection params and metadata. */
export interface RedTeamTargetDetail extends RedTeamTarget {
  connectionType?: string | null;
  apiEndpointType?: string | null;
  responseMode?: string | null;
  authType?: string | null;
  authConfig?: Record<string, unknown> | null;
  networkBrokerChannelUuid?: string | null;
  sessionSupported?: boolean;
  extraInfo?: Record<string, unknown> | null;
  description?: string | null;
  connectionParams?: Record<string, unknown>;
  background?: {
    industry?: string | null;
    use_case?: string | null;
    competitors?: string[] | null;
  };
  additionalContext?: {
    system_prompt?: string | null;
    use_case_description?: string | null;
    documents?: unknown[] | null;
  };
  metadata?: {
    multi_turn?: boolean;
    rate_limit?: number | null;
    rate_limit_error_json?: Record<string, unknown> | null;
    is_streaming_enabled?: boolean | null;
    max_turns?: number | null;
    api_endpoint_type?: string | null;
    response_mode?: string | null;
  };
}

/** Request to create a red team target. */
export interface RedTeamTargetCreateRequest {
  name: string;
  target_type: string;
  connection_params: Record<string, unknown>;
  background?: Record<string, unknown>;
  additional_context?: Record<string, unknown>;
  metadata?: Record<string, unknown>;
}

/** Request to update a red team target. */
export interface RedTeamTargetUpdateRequest {
  name?: string;
  target_type?: string;
  connection_params?: Record<string, unknown>;
  background?: Record<string, unknown>;
  additional_context?: Record<string, unknown>;
  metadata?: Record<string, unknown>;
}

/** Options for target create/update operations. */
export interface TargetOperationOptions {
  validate?: boolean;
}

/** Normalized prompt set detail. */
export interface PromptSetDetail {
  uuid: string;
  name: string;
  active: boolean;
  archive: boolean;
  description?: string;
  createdAt?: string;
  updatedAt?: string;
}

/** Prompt set version info with stats. */
export interface PromptSetVersionInfo {
  uuid: string;
  version: number;
  stats: {
    total: number;
    active: number;
    inactive: number;
  };
}

/** Normalized individual prompt. */
export interface PromptDetail {
  uuid: string;
  prompt: string;
  goal?: string;
  active: boolean;
  promptSetId: string;
}

/** Values for a single property name (SDK 0.10.0 shape). */
export interface PropertyValueList {
  name: string;
  values: string[];
}

/** Generic write-op response from custom-attack endpoints (SDK BaseResponseSchema). */
export interface MutationResponse {
  message: string;
  status?: number;
}

/** Normalized attack category with subcategories. */
export interface RedTeamCategory {
  id: string;
  displayName: string;
  description?: string;
  subCategories: Array<{
    id: string;
    displayName: string;
    description?: string;
  }>;
}

/** Normalized static report summary. */
export interface RedTeamStaticReport {
  score?: number | null;
  asr?: number | null;
  severityBreakdown: Array<{
    severity: string;
    successful: number;
    failed: number;
  }>;
  reportSummary?: string | null;
  categories: Array<{
    id: string;
    displayName: string;
    asr: number;
    successful: number;
    failed: number;
    total: number;
  }>;
}

/** Normalized dynamic scan report summary. */
export interface RedTeamDynamicReport {
  score?: number;
  asr?: number;
  totalGoals?: number;
  goalsAchieved?: number;
  totalStreams?: number;
  totalThreats?: number;
  reportSummary?: string | null;
}

/** Normalized custom attack report summary. */
export interface RedTeamCustomReport {
  totalPrompts: number;
  totalAttacks: number;
  totalThreats: number;
  failedAttacks: number;
  score: number;
  asr: number;
  promptSets: Array<{
    promptSetId: string;
    promptSetName: string;
    totalPrompts: number;
    totalAttacks: number;
    totalThreats: number;
    threatRate: number;
  }>;
}

/** Normalized attack list item (static/dynamic scans). */
export interface RedTeamAttack {
  id: string;
  name: string;
  severity?: string;
  category?: string;
  subCategory?: string;
  subCategoryDisplayName?: string;
  successful: boolean;
}

/** Normalized custom attack item (custom prompt set scans). */
export interface RedTeamCustomAttack {
  promptId: string;
  promptText: string;
  goal?: string;
  threat: boolean;
  asr?: number;
  promptSetName?: string;
}

// ---------------------------------------------------------------------------
// EULA types — normalized shapes for EULA management
// ---------------------------------------------------------------------------

/** EULA content response. */
export interface EulaContent {
  content: string;
}

/** Normalized EULA acceptance status. */
export interface EulaStatus {
  isAccepted: boolean;
  acceptedAt?: string;
  acceptedByUserId?: string;
}

// ---------------------------------------------------------------------------
// Target auth validation types
// ---------------------------------------------------------------------------

/** Request to validate target auth credentials. */
export interface TargetAuthValidationRequest {
  authType: string;
  authConfig: unknown;
  targetId?: string;
}

/** Result of target auth validation. */
export interface TargetAuthValidationResult {
  validated: boolean;
  tokenPreview?: string;
  expiresIn?: number;
}

// ---------------------------------------------------------------------------
// Instance types — normalized shapes for instance/device management
// ---------------------------------------------------------------------------

/** Request to create/update an instance. */
export interface InstanceRequest {
  tsgId: string;
  tenantId: string;
  appId: string;
  region: string;
}

/** Normalized instance response. */
export interface InstanceResponse {
  tsgId: string;
  tenantId?: string;
  appId?: string;
  isSuccess?: boolean;
}

/** Normalized instance detail (from GET). */
export interface InstanceDetail {
  tsgId: string;
  tenantId: string;
  appId: string;
  region: string;
}

/** Registry credentials. */
export interface RegistryCredentials {
  token: string;
  expiry: string;
}

// ---------------------------------------------------------------------------
// Network Broker types — normalized shapes for red team network channels
// ---------------------------------------------------------------------------

/** Normalized network broker channel. */
export interface RedTeamChannel {
  uuid?: string;
  name?: string | null;
  description?: string | null;
  status?: string | null;
  addedBy?: string | null;
  createdAt?: string | null;
  updatedAt?: string | null;
  lastOnlineAt?: string | null;
  connectedClientsCount?: number | null;
  outdatedClientsCount?: number | null;
  features?: Record<string, boolean> | null;
}

/** Filters for listing network broker channels. */
export interface RedTeamChannelListOptions {
  limit?: number;
  offset?: number;
  search?: string;
  status?: string | string[];
}

/** Request to create a network broker channel. */
export interface RedTeamChannelCreateRequest {
  name: string;
  description?: string;
}

/** Request to update a network broker channel. */
export interface RedTeamChannelUpdateRequest {
  name?: string;
  description?: string;
}

/** Normalized network broker channel statistics. */
export interface RedTeamChannelStats {
  serverDomain?: string | null;
  dockerRegistry?: string | null;
  helmChart?: string | null;
  dockerImage?: string | null;
  onlineChannels?: number | null;
  totalChannels?: number | null;
  clientVersion?: string | null;
}

// ---------------------------------------------------------------------------
// Language & error-log types — normalized shapes for red team tenant data
// ---------------------------------------------------------------------------

/** Normalized tenant language configuration. */
export interface RedTeamLanguages {
  multilingualEnabled: boolean;
  supportedJobTypes: string[];
  languages: Array<{ code: string; name: string }>;
}

/** Normalized target-profile error log entry. */
export interface RedTeamErrorLog {
  createdAt: string;
  updatedAt: string;
  jobId?: string | null;
  targetId?: string | null;
  targetVersion?: number | null;
  attackId?: string | null;
  errorType?: string | null;
  errorSource?: string | null;
  errorMessage?: string | null;
  targetObject?: Record<string, unknown> | null;
  extraInfo?: Record<string, unknown> | null;
  version?: number;
}

/** Contract for AI Red Team scan operations. */
export interface RedTeamService {
  /** Get EULA content. */
  getEulaContent(): Promise<EulaContent>;
  /** Get EULA acceptance status. */
  getEulaStatus(): Promise<EulaStatus>;
  /** Accept the EULA. */
  acceptEula(eulaContent: string): Promise<EulaStatus>;

  /** Create an instance. */
  createInstance(request: InstanceRequest): Promise<InstanceResponse>;
  /** Get instance details. */
  getInstance(tenantId: string): Promise<InstanceDetail>;
  /** Update an instance. */
  updateInstance(tenantId: string, request: InstanceRequest): Promise<InstanceResponse>;
  /** Delete an instance. */
  deleteInstance(tenantId: string): Promise<InstanceResponse>;
  /** Create devices for an instance. */
  createDevices(
    tenantId: string,
    request: Record<string, unknown>,
  ): Promise<Record<string, unknown>>;
  /** Update devices (PATCH). */
  updateDevices(
    tenantId: string,
    request: Record<string, unknown>,
  ): Promise<Record<string, unknown>>;
  /** Delete devices by serial numbers. */
  deleteDevices(tenantId: string, serialNumbers: string): Promise<Record<string, unknown>>;
  /** Get or create registry credentials. */
  getRegistryCredentials(): Promise<RegistryCredentials>;

  /** Validate target auth credentials. */
  validateTargetAuth(request: TargetAuthValidationRequest): Promise<TargetAuthValidationResult>;
  /** Get target field metadata. */
  getTargetMetadata(): Promise<Record<string, unknown>>;
  /** Get provider-specific target templates. */
  getTargetTemplates(): Promise<Record<string, unknown>>;

  /** List configured red team targets. */
  listTargets(): Promise<RedTeamTarget[]>;

  /** Get target details. */
  getTarget(uuid: string): Promise<RedTeamTargetDetail>;

  /** Create a red team target. */
  createTarget(
    request: RedTeamTargetCreateRequest,
    opts?: TargetOperationOptions,
  ): Promise<RedTeamTargetDetail>;

  /** Update a red team target. */
  updateTarget(
    uuid: string,
    request: RedTeamTargetUpdateRequest,
    opts?: TargetOperationOptions,
  ): Promise<RedTeamTargetDetail>;

  /** Delete a red team target. */
  deleteTarget(uuid: string): Promise<void>;

  /** Probe a target connection. */
  probeTarget(request: Record<string, unknown>): Promise<Record<string, unknown>>;

  /** Get target profile. */
  getTargetProfile(uuid: string): Promise<Record<string, unknown>>;

  /** Update target profile. */
  updateTargetProfile(
    uuid: string,
    request: Record<string, unknown>,
  ): Promise<Record<string, unknown>>;

  /** Create a red team scan job. */
  createScan(request: {
    name: string;
    targetUuid: string;
    jobType: string;
    categories?: Record<string, unknown>;
    customPromptSets?: string[];
    attackGoals?: string[];
    streamDepth?: number;
    streamBreadth?: number;
  }): Promise<RedTeamJob>;

  /** Get scan status by job ID. */
  getScan(jobId: string): Promise<RedTeamJob>;

  /** List recent scans with optional filters. */
  listScans(opts?: {
    status?: string;
    jobType?: string;
    targetId?: string;
    limit?: number;
  }): Promise<RedTeamJob[]>;

  /** Abort a running scan. */
  abortScan(jobId: string): Promise<void>;

  /** Get static scan report. */
  getStaticReport(jobId: string): Promise<RedTeamStaticReport>;

  /** Get dynamic scan report. */
  getDynamicReport(jobId: string): Promise<RedTeamDynamicReport>;

  /** Get custom attack report. */
  getCustomReport(jobId: string): Promise<RedTeamCustomReport>;

  /** List attacks from a static/dynamic scan. */
  listAttacks(
    jobId: string,
    opts?: { severity?: string; limit?: number },
  ): Promise<{ attacks: RedTeamAttack[]; totalItems?: number }>;

  /** List attacks from a custom prompt set scan. */
  listCustomAttacks(jobId: string, opts?: { limit?: number }): Promise<RedTeamCustomAttack[]>;

  /** List available attack categories. */
  getCategories(): Promise<RedTeamCategory[]>;

  /** Poll until scan completes. Calls onProgress for status updates. */
  waitForCompletion(
    jobId: string,
    onProgress?: (job: RedTeamJob) => void,
    intervalMs?: number,
  ): Promise<RedTeamJob>;

  /** List network broker channels. */
  listChannels(
    opts?: RedTeamChannelListOptions,
  ): Promise<{ channels: RedTeamChannel[]; totalItems?: number }>;
  /** Get a network broker channel by ID. */
  getChannel(channelId: string): Promise<RedTeamChannel>;
  /** Create a network broker channel. */
  createChannel(request: RedTeamChannelCreateRequest): Promise<RedTeamChannel>;
  /** Update a network broker channel. */
  updateChannel(channelId: string, request: RedTeamChannelUpdateRequest): Promise<RedTeamChannel>;
  /** Get network broker channel statistics. */
  getChannelStats(): Promise<RedTeamChannelStats>;

  /** List tenant languages (data plane, or management plane when `management`). */
  getLanguages(management?: boolean): Promise<RedTeamLanguages>;
  /** List target-profile error logs. */
  getTargetProfileErrorLogs(
    targetId: string,
    opts?: { limit?: number; offset?: number; search?: string },
  ): Promise<{ logs: RedTeamErrorLog[]; totalItems?: number }>;

  // Custom target adapters (SDK 0.16.0)
  listAdapters(
    opts?: RedTeamAdapterListOptions,
  ): Promise<{ adapters: RedTeamAdapterListItem[]; totalItems?: number }>;
  getAdapter(uuid: string): Promise<RedTeamAdapterDetail>;
  createAdapter(
    request: RedTeamAdapterCreateRequest,
    validate?: boolean,
  ): Promise<RedTeamAdapterDetail>;
  /** Read-modify-write: merges overrides onto the current record (upstream PUT is full-replacement). */
  updateAdapter(
    uuid: string,
    overrides: RedTeamAdapterUpdateOverrides,
    validate?: boolean,
  ): Promise<RedTeamAdapterDetail>;
  deleteAdapter(uuid: string): Promise<void>;
  /** Run a script end-to-end through the broker channel; returns an execution outcome. */
  validateAdapter(request: RedTeamAdapterValidateRequest): Promise<RedTeamAdapterValidationResult>;
}

// ---------------------------------------------------------------------------
// Model Security types — normalized shapes for model security operations
// ---------------------------------------------------------------------------

/** Normalized security group. */
export interface ModelSecurityGroup {
  uuid: string;
  name: string;
  description: string;
  sourceType: string;
  state: string;
  createdAt: string;
  updatedAt: string;
}

/** Request to create a security group. */
export interface ModelSecurityGroupCreateRequest {
  name: string;
  sourceType: string;
  description?: string;
  ruleConfigurations?: Record<string, Record<string, unknown>>;
}

/** Request to update a security group. */
export interface ModelSecurityGroupUpdateRequest {
  name?: string;
  description?: string;
}

/** Filter options for listing security groups. */
export interface ModelSecurityGroupListOptions {
  sourceTypes?: string[];
  searchQuery?: string;
  sortField?: string;
  sortDir?: string;
  enabledRules?: string[];
  skip?: number;
  limit?: number;
}

/** Normalized security rule. */
export interface ModelSecurityRule {
  uuid: string;
  name: string;
  description: string;
  ruleType: string;
  compatibleSources: string[];
  defaultState: string;
  remediation: {
    description: string;
    steps: string[];
    url: string;
  };
  editableFields: ModelSecurityRuleEditableField[];
  constantValues: Record<string, unknown>;
  defaultValues: Record<string, unknown>;
}

/** Editable field spec for a security rule. */
export interface ModelSecurityRuleEditableField {
  attributeName: string;
  type: string;
  displayName: string;
  displayType: string;
  description?: string;
  dropdownValues?: Array<{ value: string; label: string }>;
}

/** Filter options for listing security rules. */
export interface ModelSecurityRuleListOptions {
  sourceType?: string;
  searchQuery?: string;
  skip?: number;
  limit?: number;
}

/** Normalized rule instance within a security group. */
export interface ModelSecurityRuleInstance {
  uuid: string;
  securityGroupUuid: string;
  securityRuleUuid: string;
  state: string;
  createdAt: string;
  updatedAt: string;
  rule: Record<string, unknown>;
  fieldValues: Record<string, unknown>;
}

/** Filter options for listing rule instances. */
export interface ModelSecurityRuleInstanceListOptions {
  securityRuleUuid?: string;
  state?: string;
  skip?: number;
  limit?: number;
}

/** Request to update a rule instance. */
export interface ModelSecurityRuleInstanceUpdateRequest {
  state?: string;
  fieldValues?: Record<string, unknown>;
}

/** Normalized model security scan. */
export interface ModelSecurityScan {
  uuid: string;
  evalOutcome: string;
  modelUri: string;
  scanOrigin: string;
  sourceType: string;
  securityGroupName: string;
  evalSummary: {
    rulesFailed: number;
    rulesPassed: number;
    totalRules: number;
  } | null;
  createdAt: string;
  updatedAt: string;
  labels: Array<{ key: string; value: string }>;
}

/** Filter options for listing scans. */
export interface ModelSecurityScanListOptions {
  evalOutcome?: string;
  sourceType?: string;
  scanOrigin?: string;
  search?: string;
  skip?: number;
  limit?: number;
}

/** Normalized rule evaluation from a scan. */
export interface ModelSecurityEvaluation {
  uuid: string;
  result: string;
  violationCount: number;
  ruleInstanceUuid: string;
  ruleName: string;
  ruleDescription: string;
  ruleInstanceState: string;
}

/** Normalized violation from a scan. */
export interface ModelSecurityViolation {
  uuid: string;
  description: string;
  threat: string;
  threatDescription: string;
  file: string;
  ruleName: string;
  ruleDescription: string;
  ruleInstanceState: string;
}

/** Normalized scanned file from a scan. */
export interface ModelSecurityFile {
  uuid: string;
  path: string;
  type: string;
  formats: string[];
  result: string;
}

/** Filter options for listing scanned files. */
export interface ModelSecurityFileListOptions {
  type?: string;
  result?: string;
  skip?: number;
  limit?: number;
}

/** Label key-value pair. */
export interface ModelSecurityLabel {
  key: string;
  value: string;
}

/** PyPI authentication response. */
export interface ModelSecurityPyPIAuth {
  url: string;
  expiresAt: string;
}

// ---------------------------------------------------------------------------
// Model catalog types — normalized shapes for read-only model browsing
// ---------------------------------------------------------------------------

/** Normalized model catalog entry. */
export interface ModelSecurityModel {
  uuid: string;
  tsgId: string;
  name: string;
  createdAt: string;
  updatedAt: string;
  latestVersionUuid?: string | null;
  latestVersionFingerprint?: string | null;
  latestVersionRevision?: string | null;
  latestVersionHfCommitSha?: string | null;
  latestVersionOutcome?: string | null;
  latestVersionFormats?: string[] | null;
  latestVersionSourceTypes?: string[] | null;
  latestVersionScanTime?: string | null;
}

/** Filter options for listing models. */
export interface ModelSecurityModelListOptions {
  search?: string;
  searchQuery?: string;
  sortField?: string;
  sortOrder?: string;
  skip?: number;
  limit?: number;
}

/** Normalized model version. */
export interface ModelSecurityModelVersion {
  uuid: string;
  tsgId: string;
  modelUuid: string;
  revision: string;
  createdAt: string;
  updatedAt: string;
  fingerprint?: string | null;
  fileCount?: number | null;
  license?: string | null;
  latestScanTime?: string | null;
  hfCommitSha?: string | null;
  hfCommitTitle?: string | null;
  hfCommitAuthors?: string[] | null;
  hfModelName?: string | null;
  hfOrganization?: string | null;
  modelFormats?: string[] | null;
  sourceTypes?: string[] | null;
  lastEvalOutcome?: string | null;
  lastEvalSummary?: { rulesFailed: number; rulesPassed: number; totalRules: number } | null;
}

/** Filter options for listing model versions. */
export interface ModelSecurityModelVersionListOptions {
  sortOrder?: string;
  skip?: number;
  limit?: number;
}

/** Paginated list result. */
export interface PaginatedResult<T> {
  totalItems: number;
  [key: string]: T[] | number;
}

/** Contract for Model Security operations. */
export interface ModelSecurityService {
  listGroups(
    opts?: ModelSecurityGroupListOptions,
  ): Promise<{ totalItems: number; groups: ModelSecurityGroup[] }>;
  getGroup(uuid: string): Promise<ModelSecurityGroup>;
  createGroup(request: ModelSecurityGroupCreateRequest): Promise<ModelSecurityGroup>;
  updateGroup(uuid: string, request: ModelSecurityGroupUpdateRequest): Promise<ModelSecurityGroup>;
  deleteGroup(uuid: string): Promise<void>;

  listRuleInstances(
    groupUuid: string,
    opts?: ModelSecurityRuleInstanceListOptions,
  ): Promise<{ totalItems: number; ruleInstances: ModelSecurityRuleInstance[] }>;
  getRuleInstance(groupUuid: string, instanceUuid: string): Promise<ModelSecurityRuleInstance>;
  updateRuleInstance(
    groupUuid: string,
    instanceUuid: string,
    request: ModelSecurityRuleInstanceUpdateRequest,
  ): Promise<ModelSecurityRuleInstance>;

  listRules(
    opts?: ModelSecurityRuleListOptions,
  ): Promise<{ totalItems: number; rules: ModelSecurityRule[] }>;
  getRule(uuid: string): Promise<ModelSecurityRule>;

  createScan(request: Record<string, unknown>): Promise<ModelSecurityScan>;
  listScans(
    opts?: ModelSecurityScanListOptions,
  ): Promise<{ totalItems: number; scans: ModelSecurityScan[] }>;
  getScan(uuid: string): Promise<ModelSecurityScan>;

  getEvaluations(
    scanUuid: string,
    opts?: { skip?: number; limit?: number },
  ): Promise<{ totalItems: number; evaluations: ModelSecurityEvaluation[] }>;
  getEvaluation(uuid: string): Promise<ModelSecurityEvaluation>;

  getViolations(
    scanUuid: string,
    opts?: { skip?: number; limit?: number },
  ): Promise<{ totalItems: number; violations: ModelSecurityViolation[] }>;
  getViolation(uuid: string): Promise<ModelSecurityViolation>;

  getFiles(
    scanUuid: string,
    opts?: ModelSecurityFileListOptions,
  ): Promise<{ totalItems: number; files: ModelSecurityFile[] }>;

  addLabels(scanUuid: string, labels: ModelSecurityLabel[]): Promise<void>;
  setLabels(scanUuid: string, labels: ModelSecurityLabel[]): Promise<void>;
  deleteLabels(scanUuid: string, keys: string[]): Promise<void>;
  getLabelKeys(opts?: {
    skip?: number;
    limit?: number;
  }): Promise<{ totalItems: number; keys: string[] }>;
  getLabelValues(
    key: string,
    opts?: { skip?: number; limit?: number },
  ): Promise<{ totalItems: number; values: string[] }>;

  getPyPIAuth(): Promise<ModelSecurityPyPIAuth>;

  listModels(
    opts?: ModelSecurityModelListOptions,
  ): Promise<{ totalItems: number; models: ModelSecurityModel[] }>;
  getModel(uuid: string): Promise<ModelSecurityModel>;
  listModelVersions(
    modelUuid: string,
    opts?: ModelSecurityModelVersionListOptions,
  ): Promise<{ totalItems: number; versions: ModelSecurityModelVersion[] }>;
  getModelVersion(uuid: string): Promise<ModelSecurityModelVersion>;
  listModelVersionFiles(
    modelVersionUuid: string,
    opts?: { skip?: number; limit?: number },
  ): Promise<{ totalItems: number; files: ModelSecurityFile[] }>;
}

// ---------------------------------------------------------------------------
// Security profile types — normalized shapes for profile CRUD
// ---------------------------------------------------------------------------

/** Normalized security profile. */
export interface SecurityProfileInfo {
  profileId: string;
  profileName: string;
  revision?: number;
  active?: boolean;
  createdBy?: string;
  updatedBy?: string;
  lastModifiedTs?: string;
  policy?: Record<string, unknown>;
}

/** Paginated profile list result. */
export interface SecurityProfileListResult {
  profiles: SecurityProfileInfo[];
  nextOffset?: number;
}

/** Delete response from profile/topic deletion. */
export interface DeleteResponse {
  message: string;
}

/** Pagination options for list operations. */
export interface PaginationOptions {
  offset?: number;
  limit?: number;
}

// ---------------------------------------------------------------------------
// API key types
// ---------------------------------------------------------------------------

/** Normalized API key. */
export interface ApiKeyInfo {
  id: string;
  name: string;
  apiKey?: string;
  last8?: string;
  createdAt?: string;
  expiresAt?: string;
}

/** Paginated API key list. */
export interface ApiKeyListResult {
  apiKeys: ApiKeyInfo[];
  nextOffset?: number;
}

// ---------------------------------------------------------------------------
// Customer app types
// ---------------------------------------------------------------------------

/** Normalized customer app. */
export interface CustomerAppInfo {
  id?: string;
  name: string;
  description?: string;
  raw: Record<string, unknown>;
}

/** Paginated customer app list. */
export interface CustomerAppListResult {
  apps: CustomerAppInfo[];
  nextOffset?: number;
}

/**
 * Per-app consumption + violation snapshot, normalized from the SDK's dashboard endpoints.
 * Time window is fixed at construction.
 */
export interface CustomerAppConsumption {
  appId: string;
  appName: string;
  cloud?: string;
  source?: string;
  /** ISO timestamp of first monitoring (corresponds to SCM panel's "Monitoring Since"). */
  monitoringSince?: string;
  /** Attached security profile names. */
  profiles: string[];
  /** Token consumption stats with scale qualifier (K = thousands, M = millions). */
  tokens: {
    dailyAverage?: number;
    dailyAverageScale?: string;
    monthlyTotal?: number;
    monthlyTotalScale?: string;
  };
  /** Session activity counts over the window. */
  sessions: {
    total: number;
    violating: number;
  };
  /** Per-detector violation severity counts, one entry per detection_type. */
  detectors: Array<{
    type: string;
    critical: number;
    high: number;
    medium: number;
    low: number;
    total: number;
  }>;
  /** Sum of violating sessions across all detectors (mirrors SCM panel's badge). */
  totalViolating: number;
}

/** Allowed values for `--time-interval`. The API enforces this enum (other values return 400). */
export type ConsumptionTimeInterval = 7 | 30 | 60;

/** Options for {@link ManagementService.getCustomerAppConsumption}. */
export interface ConsumptionQueryOptions {
  timeInterval?: ConsumptionTimeInterval;
}

/**
 * One entry from the dashboard's apps-overview enumeration.
 *
 * One per dashboard bucket. A single registered customer-app can produce multiple buckets when
 * scan payloads sent under its API key carry different `metadata.app_name` values - the
 * dashboard tracks each as its own bucket. The `id` field is the registered `customer_appId`
 * UUID; the `name` field is the literal scan-payload value.
 */
export interface ConsumptionAppListEntry {
  /** Registered customer_appId UUID. */
  appId: string;
  /** Dashboard bucket name (literal scan-payload `metadata.app_name`). */
  appName: string;
  /** Cloud provider tag, if reported by the dashboard. */
  cloud?: string;
  /** Origin of the bucket, e.g. 'api'. */
  source?: string;
}

// ---------------------------------------------------------------------------
// Deployment profile types
// ---------------------------------------------------------------------------

/** Normalized deployment profile. */
export interface DeploymentProfileInfo {
  raw: Record<string, unknown>;
}

// ---------------------------------------------------------------------------
// Scan log types
// ---------------------------------------------------------------------------

/** Options for querying scan logs. */
export interface ScanLogQueryOptions {
  timeInterval: number;
  timeUnit: string;
  pageNumber: number;
  pageSize: number;
  filter: string;
  pageToken?: string;
}

/** Scan log query result. */
export interface ScanLogQueryResult {
  results: Record<string, unknown>[];
  pageToken?: string;
  raw: Record<string, unknown>;
}

// ---------------------------------------------------------------------------
// Management service interface
// ---------------------------------------------------------------------------

/** Contract for AIRS topic CRUD, profile CRUD, and profile linking operations. */
export interface ManagementService {
  /** Create a new custom topic. */
  createTopic(request: CreateCustomTopicRequest): Promise<SdkCustomTopic>;
  /** Update an existing custom topic by ID. */
  updateTopic(topicId: string, request: CreateCustomTopicRequest): Promise<SdkCustomTopic>;
  /** Delete a custom topic by ID. */
  deleteTopic(topicId: string): Promise<void>;
  /** Force-delete a custom topic (removes from all referencing profiles). */
  forceDeleteTopic(topicId: string, updatedBy?: string): Promise<DeleteResponse>;
  /** List all custom topics. */
  listTopics(): Promise<SdkCustomTopic[]>;
  /** Get a single custom topic by ID. */
  getTopic(topicId: string): Promise<SdkCustomTopic>;
  /** Get a single custom topic by name. */
  getTopicByName(topicName: string): Promise<SdkCustomTopic>;
  /** Assign a topic to a security profile's topic-guardrails. */
  assignTopicToProfile(
    profileName: string,
    topicId: string,
    topicName: string,
    action: 'allow' | 'block',
  ): Promise<void>;
  /** Assign multiple topics to a security profile's topic-guardrails. */
  assignTopicsToProfile(
    profileName: string,
    topics: Array<{ topicId: string; topicName: string; action: 'allow' | 'block' }>,
    guardrailAction?: 'allow' | 'block',
  ): Promise<void>;
  /** List all topics configured in a profile with full details. */
  getProfileTopics(profileName: string): Promise<ProfileTopic[]>;

  /** Get a single security profile by UUID. */
  getProfile(profileId: string): Promise<SecurityProfileInfo>;
  /** Get a single security profile by name (returns highest revision). */
  getProfileByName(profileName: string): Promise<SecurityProfileInfo>;
  /** List security profiles. */
  listProfiles(opts?: PaginationOptions): Promise<SecurityProfileListResult>;
  /** Create a security profile. */
  createProfile(request: CreateSecurityProfileRequest): Promise<SecurityProfileInfo>;
  /** Update a security profile. */
  updateProfile(
    profileId: string,
    request: CreateSecurityProfileRequest,
  ): Promise<SecurityProfileInfo>;
  /** Delete a security profile. */
  deleteProfile(profileId: string): Promise<DeleteResponse>;
  /** Force-delete a security profile (removes from referencing policies). */
  forceDeleteProfile(profileId: string, updatedBy: string): Promise<DeleteResponse>;

  // API keys
  listApiKeys(opts?: PaginationOptions): Promise<ApiKeyListResult>;
  createApiKey(request: Record<string, unknown>): Promise<ApiKeyInfo>;
  regenerateApiKey(apiKeyId: string, request: Record<string, unknown>): Promise<ApiKeyInfo>;
  deleteApiKey(apiKeyName: string, updatedBy: string): Promise<DeleteResponse>;

  // Customer apps
  listCustomerApps(opts?: PaginationOptions): Promise<CustomerAppListResult>;
  getCustomerApp(appName: string): Promise<CustomerAppInfo>;
  updateCustomerApp(appId: string, request: Record<string, unknown>): Promise<CustomerAppInfo>;
  deleteCustomerApp(appName: string, updatedBy: string): Promise<CustomerAppInfo>;
  /** Get per-app token consumption + violation breakdown from the SCM dashboard endpoints. */
  getCustomerAppConsumption(
    appName: string,
    opts?: ConsumptionQueryOptions,
  ): Promise<CustomerAppConsumption>;
  /**
   * List dashboard application buckets - the canonical apps source for consumption reporting.
   *
   * Drawn from `dashboard.applicationsOverview`. One entry per dashboard bucket, which is one
   * per distinct scan-payload `metadata.app_name` per registered customer-app. Distinct from
   * {@link ManagementService.listCustomerApps}, which enumerates registered customer-apps
   * (different granularity).
   */
  listConsumptionApps(opts?: {
    limit?: number;
    offset?: number;
  }): Promise<ConsumptionAppListEntry[]>;

  // Deployment profiles
  listDeploymentProfiles(opts?: { unactivated?: boolean }): Promise<DeploymentProfileInfo[]>;

  // Scan logs
  queryScanLogs(opts: ScanLogQueryOptions): Promise<ScanLogQueryResult>;
}

// ---------------------------------------------------------------------------
// AI Gateway
// ---------------------------------------------------------------------------

/**
 * Which plane to route an AI Gateway workspace read through.
 * `data` returns only workspaces the service account holds a workspace-scope
 * grant on; `admin` returns every workspace in the tenant.
 */
export type AiGatewayPlane = 'data' | 'admin';

/** Normalized AI Gateway workspace list row. */
export interface AiGatewayWorkspace {
  id: string;
  slug: string;
  name: string;
  icon?: string | null;
  description?: string | null;
  createdAt?: string;
  lastUpdatedAt?: string;
  isDefault: boolean;
  /**
   * Lifecycle state. `get` can report `null` for a workspace `list` calls
   * `active` — treat `null` as "unknown", never "inactive".
   */
  status?: string | null;
  /** SCM role scope granting data-plane access to this workspace. */
  scopeName?: string;
}

/** Normalized AI Gateway workspace detail (list row + settings blocks). */
export interface AiGatewayWorkspaceDetail extends AiGatewayWorkspace {
  defaults?: Record<string, unknown> | null;
  /** Usage-limit policies, always an array (legacy single-object form is wrapped). */
  usageLimits: Array<Record<string, unknown>>;
  /** Rate-limit policies, always an array (legacy single-object form is wrapped). */
  rateLimits: Array<Record<string, unknown>>;
  securitySettings?: Record<string, boolean>;
  dataPlaneSecuritySettings?: Record<string, unknown>;
  settings?: Record<string, unknown>;
}

export interface AiGatewayWorkspaceListOptions {
  plane?: AiGatewayPlane;
  /** Omitting this returns active workspaces only — archived rows are hidden. */
  status?: 'active' | 'archived';
}

export interface AiGatewayWorkspaceGetOptions {
  plane?: AiGatewayPlane;
}

/** Service interface for AI Gateway operations used by the CLI. */
export interface AiGatewayService {
  listWorkspaces(opts?: AiGatewayWorkspaceListOptions): Promise<AiGatewayWorkspace[]>;
  /** Merge an active and an archived admin-plane read — no single call returns both. */
  listAllWorkspaces(): Promise<AiGatewayWorkspace[]>;
  getWorkspace(
    workspaceRef: string,
    opts?: AiGatewayWorkspaceGetOptions,
  ): Promise<AiGatewayWorkspaceDetail>;
  /** Create a workspace (admin plane); renders from a follow-up get, not the write response. */
  createWorkspace(request: AiGatewayWorkspaceCreateRequest): Promise<AiGatewayWorkspaceDetail>;
  /** Partial update (admin plane); the API returns `{}`, so the result comes from a re-read. */
  updateWorkspace(
    workspaceRef: string,
    request: AiGatewayWorkspaceUpdateRequest,
  ): Promise<AiGatewayWorkspaceDetail>;
  /** Soft delete — archives the workspace; there is no hard delete. */
  deleteWorkspace(workspaceRef: string): Promise<void>;
  /** Total and per-day spend for a workspace. Values are CENTS. */
  getTelemetryCost(opts: AiGatewayCostOptions): Promise<AiGatewayCostReport>;
}

/** Request to create an AI Gateway workspace. */
export interface AiGatewayWorkspaceCreateRequest {
  name: string;
  /**
   * SCM role scope granting data-plane access, e.g. `ws_production_bx7qw0`.
   * Required and not derived from `name` — a workspace created with a scope
   * nobody holds is invisible to data-plane lists.
   */
  scopeName: string;
  description?: string;
  icon?: string;
  defaults?: Record<string, unknown>;
  users?: string[];
  usageLimits?: Array<Record<string, unknown>>;
  rateLimits?: Array<Record<string, unknown>>;
}

/** Partial update for an AI Gateway workspace — send only what changes. */
export interface AiGatewayWorkspaceUpdateRequest {
  name?: string;
  description?: string;
  icon?: string;
  defaults?: Record<string, unknown>;
  usageLimits?: Array<Record<string, unknown>>;
  rateLimits?: Array<Record<string, unknown>>;
}

/** Options for the AI Gateway telemetry cost query. */
export interface AiGatewayCostOptions {
  /** Workspace slug (not UUID) — required by every telemetry endpoint. */
  workspaceSlug: string;
  /** Rolling window in days, counted back from now. Defaults to 7. */
  days?: number;
}

/** Normalized AI Gateway cost report. All monetary values are CENTS — the API never converts. */
export interface AiGatewayCostReport {
  workspaceSlug: string;
  days: number;
  totalCents: number;
  avgCents: number;
  quotaExceeded: boolean;
  records: Array<{ date: string; costCents: number }>;
}

// ---------------------------------------------------------------------------
// Red Team custom target adapters (SDK 0.16.0)
// ---------------------------------------------------------------------------

/** An adapter configuration variable. Secrets are masked; key off `isRedacted`, not the value. */
export interface RedTeamAdapterVar {
  key: string;
  value?: string | null;
  type: 'VAR' | 'SECRET';
  isRedacted?: boolean;
}

/** Adapter list row — no script, description, or variables; `get` for the full record. */
export interface RedTeamAdapterListItem {
  uuid: string;
  name: string;
  status: string;
  createdAt?: string;
  updatedAt?: string;
  createdByUserId?: string | null;
  targetCount?: number | null;
}

/** Full adapter record. */
export interface RedTeamAdapterDetail {
  uuid: string;
  tsgId?: string;
  name: string;
  scriptB64: string;
  status: string;
  description?: string | null;
  networkBrokerChannelUuid?: string | null;
  variables: RedTeamAdapterVar[];
  targetCount?: number | null;
  createdAt?: string | null;
  updatedAt?: string | null;
  createdByUserId?: string | null;
  updatedByUserId?: string | null;
}

export interface RedTeamAdapterListOptions {
  limit?: number;
  offset?: number;
  search?: string;
}

export interface RedTeamAdapterCreateRequest {
  name: string;
  scriptB64: string;
  /** Sample prompt used to exercise the adapter during validation. Not stored. */
  prompt: string;
  description?: string;
  /** Optional while DRAFT; required to activate (validate: true). */
  networkBrokerChannelUuid?: string;
  variables?: RedTeamAdapterVar[];
}

/**
 * CLI-side overrides for adapter update. The upstream PUT is a full
 * replacement, so the service merges these onto the current record —
 * `prompt` is the only always-required field because it is never stored.
 */
export interface RedTeamAdapterUpdateOverrides {
  prompt: string;
  name?: string;
  scriptB64?: string;
  description?: string;
  networkBrokerChannelUuid?: string;
  /** Replaces the WHOLE variable set when given; omitted keys are deleted upstream. */
  variables?: RedTeamAdapterVar[];
}

export interface RedTeamAdapterValidateRequest {
  scriptB64: string;
  networkBrokerChannelUuid: string;
  prompt: string;
  variables?: RedTeamAdapterVar[];
  /** Resolve redacted/null variable values from this stored adapter before the run. */
  adapterUuid?: string;
}

/** Execution outcome of a validation run — not an adapter record. */
export interface RedTeamAdapterValidationResult {
  validated: boolean;
  stdout?: string | null;
  stderr?: string | null;
  traceback?: string | null;
}
