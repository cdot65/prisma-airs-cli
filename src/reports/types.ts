import type { ManagementClient } from '@cdot65/prisma-airs-sdk';

/** Read-only SDK surface needed by the first product report. */
export interface RuntimeReportClient {
  dashboard: Pick<ManagementClient['dashboard'], 'applicationsOverview'>;
  profiles: Pick<ManagementClient['profiles'], 'list'>;
  customerApps: Pick<ManagementClient['customerApps'], 'list'>;
  scanLogs: Pick<ManagementClient['scanLogs'], 'query'>;
}

export type ReportSourceStatus = 'complete' | 'partial' | 'unavailable';
export type ReportPriority = 'attention' | 'review' | 'info';
export type ReportFormat = 'html' | 'markdown';

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

export interface ReportLogSummary {
  entries: number;
  actions: Array<{ name: string; count: number }>;
  verdicts: Array<{ name: string; count: number }>;
  tokens: number | null;
  missingTimestamps: number;
  outsideWindow: number;
}

/** Allowlisted, credential-free projection; never contains raw responses or scan content. */
export interface RuntimeDailyReport {
  schemaVersion: 1;
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
  logs: ReportLogSummary;
  limitations: string[];
}

export interface RuntimeReportOptions {
  title?: string;
  /** Per-source page budget (1–100); partial results are explicitly marked. Default 10. */
  maxPages?: number;
  /** Inject a clock for deterministic tests. */
  now?: () => Date;
}
