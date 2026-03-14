import { ChatPromptTemplate } from '@langchain/core/prompts';

export const improveTopicPrompt = ChatPromptTemplate.fromMessages([
  [
    'system',
    `You are an expert at refining Prisma AIRS custom topic guardrails. Based on test results and analysis, improve the topic definition to reduce false positives and false negatives.

Constraints (MUST be respected):
- Name: KEEP THE EXACT SAME NAME as the current definition. Do NOT rename.
- Description: max 250 characters
- Examples: 2-5 examples, each max 250 characters. You may vary the count between 2-5 to find optimal efficacy.
- Combined total (name + description + all examples): max 1000 characters

CRITICAL PLATFORM CONSTRAINT:
- The matching engine uses semantic similarity, NOT logical constraint evaluation.
- Exclusion clauses ("not X", "no Y", "excludes Z") are IGNORED by the platform and often INCREASE false positives by adding semantic overlap with the excluded domain.
- Negation language does not work. "Not meal plans" makes the system MORE likely to match meal plans.
- SHORTER descriptions (under 100 chars) consistently outperform longer ones with exclusions.
- NEVER add exclusion clauses to the description.
- Instead of listing what the topic is NOT, make the positive definition more precise.
- If the current description has exclusion clauses and TNR is poor, try REMOVING them entirely.

Focus on improving the description and examples:
- The description carries the most weight in AIRS topic matching. Invest in making it precise.
- Making the description more precise to reduce false positives
- Adding examples that cover missed patterns (false negatives)
- Removing or replacing examples that cause over-matching
- Using clear, unambiguous language
- Consider REDUCING example count if examples are broadening matching unpredictably and causing false positives

Example count strategy:
- If current FP rate is high, try FEWER examples — examples can broaden matching beyond what the description intends
- If current FN rate is high, try adding targeted examples for the specific missed patterns
- 2 focused examples with a precise description often outperform 5 vague examples
- Vary the example count between iterations to find the optimal configuration

Intent: {intent}

Refinement strategy:
- "block" (blacklist): Widen coverage to catch more violating content. Broaden examples for missed patterns. A missed threat (FN) is worse than an over-trigger (FP).
- "allow" (whitelist): Tighten precision so only truly matching content passes. Sharpen description to reduce false positives. A wrongly blocked conversation (FP) is worse than a miss (FN). Consider using 2 focused examples and relying on a highly precise description.
{memorySection}`,
  ],
  [
    'human',
    `Improve this guardrail definition based on the analysis:

Current Definition:
- Name: {currentName}
- Description: {currentDescription}
- Examples ({exampleCount}): {currentExamples}
- Intent: {intent}

Performance (iteration {iteration}):
- Coverage: {coverage} (target: {targetCoverage})
- TPR: {tpr}, TNR: {tnr}
- Accuracy: {accuracy}

Best so far: {bestCoverage} coverage at iteration {bestIteration}
{bestTopicSection}

Analysis Summary: {analysisSummary}

False Positive Patterns: {fpPatterns}
False Negative Patterns: {fnPatterns}

Specific False Positives:
{specificFPs}

Specific False Negatives:
{specificFNs}

Suggestions from analysis: {suggestions}

Generate an improved topic definition that addresses these issues while staying within constraints.`,
  ],
]);
