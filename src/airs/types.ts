/**
 * AIRS integration types — scan results and service interfaces for the
 * Prisma AIRS scanner, topic management, and red team APIs.
 */

import type {
  CreateCustomTopicRequest,
  CreateSecurityProfileRequest,
  CustomTopic as SdkCustomTopic,
} from '@cdot65/prisma-airs-sdk';
import type { ProfileTopic } from '../audit/types.js';

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
}

/** Contract for runtime scanning operations (sync + async). */
export interface RuntimeService {
  /** Scan a single prompt (and optional response) synchronously. */
  scanPrompt(profileName: string, prompt: string, response?: string): Promise<RuntimeScanResult>;
  /** Submit prompts for async bulk scanning, returns scan IDs. */
  submitBulkScan(profileName: string, prompts: string[]): Promise<string[]>;
  /** Poll async scan results until all complete. */
  pollResults(scanIds: string[], intervalMs?: number): Promise<RuntimeScanResult[]>;
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
  getPropertyNames(): Promise<PropertyName[]>;
  /** Create a property name. */
  createPropertyName(name: string): Promise<PropertyName>;
  /** Get values for a property. */
  getPropertyValues(name: string): Promise<PropertyValue[]>;
  /** Create a property value. */
  createPropertyValue(name: string, value: string): Promise<PropertyValue>;
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

/** Property name entry. */
export interface PropertyName {
  name: string;
}

/** Property value entry. */
export interface PropertyValue {
  name: string;
  value: string;
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

/** Contract for AI Red Team scan operations. */
export interface RedTeamService {
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

  /** Get custom attack report. */
  getCustomReport(jobId: string): Promise<RedTeamCustomReport>;

  /** List attacks from a static/dynamic scan. */
  listAttacks(
    jobId: string,
    opts?: { severity?: string; limit?: number },
  ): Promise<RedTeamAttack[]>;

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

// ---------------------------------------------------------------------------
// Deployment profile types
// ---------------------------------------------------------------------------

/** Normalized deployment profile. */
export interface DeploymentProfileInfo {
  raw: Record<string, unknown>;
}

// ---------------------------------------------------------------------------
// DLP profile types
// ---------------------------------------------------------------------------

/** Normalized DLP profile. */
export interface DlpProfileInfo {
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

  // Deployment profiles
  listDeploymentProfiles(opts?: { unactivated?: boolean }): Promise<DeploymentProfileInfo[]>;

  // DLP profiles
  listDlpProfiles(): Promise<DlpProfileInfo[]>;

  // Scan logs
  queryScanLogs(opts: ScanLogQueryOptions): Promise<ScanLogQueryResult>;
}
