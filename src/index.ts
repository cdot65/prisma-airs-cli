/**
 * Prisma AIRS CLI — Public library API
 *
 * Automated generation, testing, and iterative refinement of
 * Palo Alto Prisma AIRS custom topic guardrails.
 */

// ---------------------------------------------------------------------------
// AIRS integration — scan prompts and manage topics/profiles via SDK
// ---------------------------------------------------------------------------
export { SdkManagementService } from './airs/management.js';
export { SdkModelSecurityService } from './airs/modelsecurity.js';
export { SdkPromptSetService } from './airs/promptsets.js';
export { SdkRedTeamService } from './airs/redteam.js';
export type { PollRetryOptions } from './airs/runtime.js';
export { SDK_ASYNC_BATCH_SIZE, SdkRuntimeService } from './airs/runtime.js';
export { AirsScanService } from './airs/scanner.js';
// ---------------------------------------------------------------------------
// Core loop & metrics — the main generate→test→evaluate→improve cycle
// ---------------------------------------------------------------------------
export type {
  ApiKeyInfo,
  ApiKeyListResult,
  BatchEntry,
  BulkScanAction,
  BulkScanResult,
  ConsumptionAppListEntry,
  ConsumptionQueryOptions,
  ConsumptionTimeInterval,
  CustomerAppConsumption,
  CustomerAppInfo,
  CustomerAppListResult,
  DeleteResponse,
  DeploymentProfileInfo,
  EulaContent,
  EulaStatus,
  IndexedPrompt,
  InstanceDetail,
  InstanceRequest,
  InstanceResponse,
  ManagementService,
  ModelSecurityEvaluation,
  ModelSecurityFile,
  ModelSecurityFileListOptions,
  ModelSecurityGroup,
  ModelSecurityGroupCreateRequest,
  ModelSecurityGroupListOptions,
  ModelSecurityGroupUpdateRequest,
  ModelSecurityLabel,
  ModelSecurityModel,
  ModelSecurityModelListOptions,
  ModelSecurityModelVersion,
  ModelSecurityModelVersionListOptions,
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
  MutationResponse,
  PaginationOptions,
  ProfileTopic,
  PromptDetail,
  PromptSetDetail,
  PromptSetService,
  PromptSetVersionInfo,
  PropertyValueList,
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
  ReliableRuntimeService,
  RuntimeScanResult,
  RuntimeService,
  ScanLogQueryOptions,
  ScanLogQueryResult,
  ScanResult,
  ScanService,
  SecurityProfileInfo,
  SecurityProfileListResult,
  SubmittedBatch,
  TargetAuthValidationRequest,
  TargetAuthValidationResult,
  TargetOperationOptions,
} from './airs/types.js';
// ---------------------------------------------------------------------------
// Backup — export/import AIRS configuration to/from local files
// ---------------------------------------------------------------------------
export {
  readBackupDir,
  readBackupFile,
  resolveOutputDir,
  sanitizeFilename,
  writeBackupFile,
} from './backup/io.js';
export type {
  BackupEnvelope,
  BackupFormat,
  BackupResult,
  ResourceType,
  RestoreResult,
} from './backup/types.js';
// ---------------------------------------------------------------------------
// Config — cascading config loader (CLI > env > file > Zod defaults)
// ---------------------------------------------------------------------------
export { loadConfig } from './config/loader.js';
// ---------------------------------------------------------------------------
// AIRS constraints — validation helpers enforcing Prisma AIRS topic limits
// ---------------------------------------------------------------------------
export type { ValidationError } from './core/constraints.js';
export {
  validateDescription,
  validateExamples,
  validateName,
  validateTopic,
} from './core/constraints.js';
export { computeCategoryBreakdown, computeMetrics } from './core/metrics.js';
export type {
  AnalysisReport,
  CategoryBreakdown,
  CustomTopic,
  EfficacyMetrics,
  IterationResult,
  RunState,
  TestCase,
  TestResult,
  UserInput,
} from './core/types.js';
