import type { MemoryStore } from './store.js';
import type { Learning } from './types.js';

/**
 * Builds a budget-aware memory section for LLM prompt injection.
 * Retrieves relevant learnings from the store, sorts by corroboration count,
 * and formats them within a character budget (verbose -> compact -> omit).
 */
export class MemoryInjector {
  /**
   * @param store - Memory store for retrieving learnings.
   * @param maxChars - Character budget for the memory section. Default 3000.
   */
  constructor(
    private store: MemoryStore,
    private maxChars: number = 3000,
  ) {}

  /**
   * Build a formatted memory section string for prompt injection.
   * @param topicDescription - Topic to find relevant learnings for.
   * @returns Formatted learnings string, or empty string if none found.
   */
  async buildMemorySection(topicDescription: string): Promise<string> {
    const memories = await this.store.findRelevant(topicDescription);
    if (memories.length === 0) return '';

    const allLearnings: Learning[] = [];
    const allAntiPatterns: string[] = [];

    for (const mem of memories) {
      allLearnings.push(...mem.learnings);
      allAntiPatterns.push(...mem.antiPatterns);
    }

    if (allLearnings.length === 0 && allAntiPatterns.length === 0) return '';

    // Sort by corroborations descending
    allLearnings.sort((a, b) => b.corroborations - a.corroborations);

    const header = [
      '\n## Learnings from Previous Runs',
      'These are empirically validated observations from prior guardrail generation runs:',
    ];
    // Budget for content after header; each added line costs lineLen + 1 (newline separator)
    let budget = this.maxChars - header.join('\n').length;

    const lines: string[] = [...header];
    let compactMode = false;
    let omitted = 0;

    for (const l of allLearnings) {
      const tag = l.outcome === 'degraded' ? 'AVOID' : 'DO';
      const seenCount = l.corroborations + 1;

      if (!compactMode) {
        const verbose = `- [${tag}] ${l.insight} (${l.changeType}, seen ${seenCount}x)`;
        if (verbose.length + 1 <= budget) {
          lines.push(verbose);
          budget -= verbose.length + 1; // +1 for newline
          continue;
        }
        // Switch to compact mode
        compactMode = true;
      }

      const compact = `- [${tag}] ${l.insight}`;
      if (compact.length + 1 <= budget) {
        lines.push(compact);
        budget -= compact.length + 1;
      } else {
        omitted++;
      }
    }

    if (omitted > 0) {
      const notice = `(+${omitted} more learnings omitted)`;
      if (notice.length + 1 <= budget) {
        lines.push(notice);
        budget -= notice.length + 1;
      }
    }

    const uniqueAntiPatterns = [...new Set(allAntiPatterns)];
    if (uniqueAntiPatterns.length > 0) {
      lines.push('');
      lines.push('Known anti-patterns:');
      budget -= 1 + 'Known anti-patterns:'.length + 1; // blank line + header

      for (const ap of uniqueAntiPatterns) {
        const line = `- ${ap}`;
        if (line.length + 1 <= budget) {
          lines.push(line);
          budget -= line.length + 1;
        }
      }
    }

    return lines.join('\n');
  }
}
