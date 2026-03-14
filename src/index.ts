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
export { SdkRuntimeService } from './airs/runtime.js';
export { AirsScanService } from './airs/scanner.js';
// ---------------------------------------------------------------------------
// Core loop & metrics — the main generate→test→evaluate→improve cycle
// ---------------------------------------------------------------------------
export type {
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
  PromptDetail,
  PromptSetDetail,
  PromptSetService,
  PromptSetVersionInfo,
  PropertyName,
  PropertyValue,
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
  RuntimeScanResult,
  RuntimeService,
  TargetOperationOptions,
} from './airs/types.js';
// ---------------------------------------------------------------------------
// Audit — profile-level multi-topic evaluation and conflict detection
// ---------------------------------------------------------------------------
export {
  computeCompositeMetrics,
  computeTopicAuditResults,
  detectConflicts,
} from './audit/evaluator.js';
export { buildAuditReportHtml, buildAuditReportJson } from './audit/report.js';
export { runAudit } from './audit/runner.js';
export type {
  AuditEvent,
  AuditResult,
  ConflictPair,
  ProfileTopic,
  TopicAuditResult,
} from './audit/types.js';
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
export type { LlmService, LoopDependencies } from './core/loop.js';
export { runLoop } from './core/loop.js';
export { computeCategoryBreakdown, computeMetrics } from './core/metrics.js';
export type {
  AnalysisReport,
  CategoryBreakdown,
  CustomTopic,
  EfficacyMetrics,
  IterationResult,
  LoopEvent,
  RunState,
  TestCase,
  TestResult,
  UserInput,
} from './core/types.js';
// ---------------------------------------------------------------------------
// LLM — provider factory and structured-output service for topic generation
// ---------------------------------------------------------------------------
export { createLlmProvider } from './llm/provider.js';
export { LangChainLlmService } from './llm/service.js';
// ---------------------------------------------------------------------------
// Memory — cross-run learning persistence, extraction, and prompt injection
// ---------------------------------------------------------------------------
export { LearningExtractor } from './memory/extractor.js';
export { MemoryInjector } from './memory/injector.js';
export { MemoryStore, normalizeCategory } from './memory/store.js';
export type {
  IterationDiff,
  Learning,
  TopicMemory,
} from './memory/types.js';
// ---------------------------------------------------------------------------
// Persistence — save/load/list run state as JSON for resume & reporting
// ---------------------------------------------------------------------------
export { JsonFileStore } from './persistence/store.js';

// ---------------------------------------------------------------------------
// Reports — structured evaluation report generation (JSON/HTML)
// ---------------------------------------------------------------------------
export { buildReportHtml } from './report/html.js';
export { buildReportJson } from './report/json.js';
export type {
  IterationSummary,
  MetricsDelta,
  ReportOutput,
  RunDiff,
  RunSummary,
  TestDetail,
} from './report/types.js';
