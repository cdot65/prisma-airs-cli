import { ChatPromptTemplate } from '@langchain/core/prompts';

export const extractLearningsPrompt = ChatPromptTemplate.fromMessages([
  [
    'system',
    `You are analyzing the results of a Prisma AIRS custom topic guardrail generation run to extract reusable learnings.

Given the iteration history, identify:
1. **Learnings**: What worked, what didn't, and why. Each learning should have:
   - insight: A concise observation (e.g., "Short direct descriptions outperform nuanced ones on AIRS")
   - strategy: What was actually done (e.g., "Used a short description without exclusion clauses")
   - outcome: "improved", "degraded", or "neutral"
   - changeType: "description-only", "examples-only", "both", or "initial"
   - tags: keywords like "brevity", "coded-language", "fp-reduction", "fn-reduction", "exclusion-clauses", "example-count", "zero-examples", "description-weight"

2. **Anti-patterns**: Strategies that consistently degraded performance (e.g., "Adding coded language patterns broadens AIRS matching unpredictably")

Pay special attention to the RELATIONSHIP BETWEEN EXAMPLE COUNT AND EFFICACY:
- Track whether reducing examples improved or degraded metrics
- Note when fewer examples (2) outperformed configurations with more examples (3-5)
- Identify if specific example counts (2, 3, 4, 5) correlate with better coverage for this topic type
- Tag learnings about example count with "example-count" and the relevant count (e.g., "few-examples", "many-examples")

Focus on actionable, generalizable insights. Avoid restating metrics — explain the *why* behind changes.`,
  ],
  [
    'human',
    `Analyze this guardrail generation run:

Topic Description: {topicDescription}
Intent: {intent}
Total Iterations: {totalIterations}
Best Iteration: {bestIteration} (coverage: {bestCoverage})

Iteration History:
{iterationHistory}

Extract structured learnings from this run.`,
  ],
]);
