import type { RunState } from '../core/types.js';

/** Contract for run state persistence backends. */
export interface RunStore {
  /** Save or overwrite a run state. */
  save(run: RunState): Promise<void>;
  /** Load a run state by ID, or null if not found. */
  load(id: string): Promise<RunState | null>;
  /** List summaries of all persisted runs. */
  list(): Promise<RunStateSummary[]>;
  /** Delete a run state by ID. */
  delete(id: string): Promise<void>;
}

/** Lightweight summary of a persisted run for listing. */
export interface RunStateSummary {
  id: string;
  createdAt: string;
  updatedAt: string;
  status: RunState['status'];
  currentIteration: number;
  bestCoverage: number;
  topicDescription: string;
}
