/**
 * Profile audit types — multi-topic evaluation, per-topic metrics, conflict detection.
 */

import type { EfficacyMetrics, TestResult } from '../core/types.js';

/** Enriched topic entry read from a profile's policy. */
export interface ProfileTopic {
  topicId: string;
  topicName: string;
  action: 'allow' | 'block';
  description: string;
  examples: string[];
}

/** Per-topic evaluation result. */
export interface TopicAuditResult {
  topic: ProfileTopic;
  testResults: TestResult[];
  metrics: EfficacyMetrics;
}

/** Detected conflict between two topics. */
export interface ConflictPair {
  topicA: string;
  topicB: string;
  description: string;
  evidence: string[];
}

/** Complete audit result for a profile. */
export interface AuditResult {
  profileName: string;
  timestamp: string;
  topics: TopicAuditResult[];
  compositeMetrics: EfficacyMetrics;
  conflicts: ConflictPair[];
}

/** Audit event union yielded by the audit runner. */
export type AuditEvent =
  | { type: 'topics:loaded'; topics: ProfileTopic[] }
  | { type: 'tests:generated'; topicName: string; count: number }
  | { type: 'scan:progress'; completed: number; total: number }
  | { type: 'evaluate:complete'; result: AuditResult }
  | { type: 'audit:complete'; result: AuditResult };
