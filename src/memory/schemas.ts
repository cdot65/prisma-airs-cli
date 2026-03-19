import { z } from 'zod';

export const ExtractedLearningSchema = z.object({
  insight: z.string().min(1),
  strategy: z.string().min(1),
  outcome: z.enum(['improved', 'degraded', 'neutral']),
  changeType: z.enum(['description-only', 'examples-only', 'both', 'initial']).optional(),
  tags: z.array(z.string()),
});

export const LearningExtractionOutputSchema = z.object({
  learnings: z.array(ExtractedLearningSchema),
  antiPatterns: z.array(z.string()),
});

export type ExtractedLearning = z.infer<typeof ExtractedLearningSchema>;
export type LearningExtractionOutput = z.infer<typeof LearningExtractionOutputSchema>;
