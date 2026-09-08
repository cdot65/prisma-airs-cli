import type { ManagementClient } from '@cdot65/prisma-airs-sdk';

/** Read-only SDK surface needed by the first product report. */
export interface RuntimeReportClient {
  dashboard: Pick<
    ManagementClient['dashboard'],
    | 'applicationsOverview'
    | 'sessionsOverview'
    | 'sessionsChart'
    | 'topApplicationsViolations'
    | 'applicationsViolationsTrend'
  >;
  profiles: Pick<ManagementClient['profiles'], 'list'>;
  customerApps: Pick<ManagementClient['customerApps'], 'list'>;
}

export type ReportSourceStatus = 'complete' | 'partial' | 'unavailable';
export type ReportPriority = 'attention' | 'review' | 'info';
export type ReportFormat = 'html' | 'markdown';

export interface EnvironmentReportTable {
  title: string;
  note: string;
  headers: string[];
  rows: Array<Array<string | number | null>>;
}

/** Aggregate-only view shared by portable product dashboards. */
export interface EnvironmentReport {
  schemaVersion: 1;
  product: string;
  title: string;
  collectionStartedAt: string;
  generatedAt: string;
  window: { start: string; end: string };
  windowLabel?: string;
  schemaLabel?: string;
  health: 'attention' | 'review' | 'no-findings' | 'unknown';
  sources: ReportSource[];
  findings: ReportFinding[];
  tables: EnvironmentReportTable[];
  limitations: string[];
}

export interface ReportSource {
  name: string;
  method: string;
  window: string;
  status: ReportSourceStatus;
  records: number;
  pages: number;
  notes: string[];
}

export interface ReportFinding {
  priority: ReportPriority;
  title: string;
  evidence: string;
  recommendation: string;
  source: string;
}

export interface ReportApplication {
  name: string;
  sessions: number | null;
  violatingSessions: number | null;
  registered: boolean | null;
}

export interface ReportProfile {
  name: string;
  revision: number | null;
  active: boolean | null;
  modified: string | null;
  timeoutActions: string[];
  storageMasking: string;
}

export interface ReportRegisteredApp {
  name: string;
  environment: string;
  cloud: string;
  model: string;
  keyAssociations: number | null;
}

export interface ReportSessionSummary {
  entries: number | null;
  statuses: Array<{ name: string; count: number }>;
  violatingSessions: number | null;
  missingTimestamps: number;
  outsideWindow: number;
}

export interface ReportSeverity {
  critical: number | null;
  high: number | null;
  medium: number | null;
  low: number | null;
  total: number | null;
}

export interface ReportDailyTelemetry {
  chart: {
    sessions: number | null;
    violatingSessions: number | null;
    buckets: Array<{
      time: string;
      sessions: number | null;
      violatingSessions: number | null;
      violations: ReportSeverity;
    }>;
  };
  topApplications: Array<{
    name: string;
    violations: number | null;
    detectors: Array<{ name: string; count: number | null }>;
  }>;
  violationTrend: Array<{ time: string; violations: ReportSeverity }>;
}

/** Allowlisted, credential-free projection; never contains raw responses or scan content. */
export interface RuntimeDailyReport {
  schemaVersion: 2;
  product: 'Prisma AIRS AI Runtime Security';
  title: string;
  generatedAt: string;
  collectionStartedAt: string;
  window: { start: string; end: string; description: string };
  health: 'attention' | 'review' | 'no-findings' | 'unknown';
  sources: ReportSource[];
  findings: ReportFinding[];
  activity: {
    sessions: number | null;
    violatingSessions: number | null;
    violationRate: number | null;
    applications: ReportApplication[];
  };
  profiles: ReportProfile[];
  registeredApps: ReportRegisteredApp[];
  sessions: ReportSessionSummary;
  dailyTelemetry: ReportDailyTelemetry;
  limitations: string[];
}

export interface RuntimeReportOptions {
  title?: string;
  /** Per-source page budget (1–100); partial results are explicitly marked. Default 40. */
  maxPages?: number;
  /** Inject a clock for deterministic tests. */
  now?: () => Date;
}
